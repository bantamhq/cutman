#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR"
VARS_FILE="$PROJECT_ROOT/tests/api/vars.env"
SERVER_PID=""
DATA_DIR=""

cleanup() {
    echo "Cleaning up..."
    if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi
    if [[ -n "$DATA_DIR" && -d "$DATA_DIR" ]]; then
        rm -rf "$DATA_DIR"
    fi
    rm -f "$VARS_FILE"
}
trap cleanup EXIT

echo "=== Building server (release mode) ==="
cd "$PROJECT_ROOT"
cargo build --release

echo "=== Setting up test environment ==="
DATA_DIR=$(mktemp -d)
HOST="127.0.0.1"
PORT="18080"

BASE_URL="http://${HOST}:${PORT}"

echo "Data directory: $DATA_DIR"
echo "Server URL: $BASE_URL"

echo "=== Starting server ==="
"$PROJECT_ROOT/target/release/cutman" serve --data-dir "$DATA_DIR" --host "$HOST" --port "$PORT" &
SERVER_PID=$!

# Wait for server to start
sleep 2

# Check if server is running
if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "ERROR: Server failed to start"
    exit 1
fi

# Wait for server to be ready
for i in {1..30}; do
    if curl -s "$BASE_URL/health" > /dev/null 2>&1; then
        echo "Server is ready"
        break
    fi
    if [[ $i -eq 30 ]]; then
        echo "ERROR: Server did not become ready in time"
        exit 1
    fi
    sleep 0.5
done

# Read admin token from the token file
ADMIN_TOKEN=$(cat "$DATA_DIR/.admin_token" 2>/dev/null || echo "")
if [[ -z "$ADMIN_TOKEN" ]]; then
    echo "ERROR: Could not find admin token"
    exit 1
fi
echo "Admin token captured"

echo "=== Creating test data ==="

# Create test namespace
NAMESPACE_RESPONSE=$(curl -s -X POST "$BASE_URL/api/v1/admin/namespaces" \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"name": "test-namespace", "repo_limit": 100}')
NAMESPACE_ID=$(echo "$NAMESPACE_RESPONSE" | jq -r '.data.id')
echo "Created namespace: $NAMESPACE_ID"

# Create test user
USER_RESPONSE=$(curl -s -X POST "$BASE_URL/api/v1/admin/users" \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"namespace_name": "test-user"}')
USER_ID=$(echo "$USER_RESPONSE" | jq -r '.data.id')
USER_NS_ID=$(echo "$USER_RESPONSE" | jq -r '.data.primary_namespace_id')
echo "Created user: $USER_ID (namespace: $USER_NS_ID)"

# Grant user access to test-namespace
curl -s -X POST "$BASE_URL/api/v1/admin/users/$USER_ID/namespace-grants" \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"namespace_id\": \"$NAMESPACE_ID\", \"allow\": [\"namespace:read\", \"namespace:write\", \"repo:read\", \"repo:write\", \"repo:admin\"]}" > /dev/null

# Create user token
TOKEN_RESPONSE=$(curl -s -X POST "$BASE_URL/api/v1/admin/users/$USER_ID/tokens" \
    -H "Authorization: Bearer $ADMIN_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"description": "Test token"}')
USER_TOKEN=$(echo "$TOKEN_RESPONSE" | jq -r '.data.token')
TOKEN_ID=$(echo "$TOKEN_RESPONSE" | jq -r '.data.metadata.id')
echo "Created user token"

# Create test repo
REPO_RESPONSE=$(curl -s -X POST "$BASE_URL/api/v1/repos" \
    -H "Authorization: Bearer $USER_TOKEN" \
    -H "Content-Type: application/json" \
    -d '{"name": "test-repo", "description": "Test repository", "namespace": "test-user"}')
REPO_ID=$(echo "$REPO_RESPONSE" | jq -r '.data.id')
echo "Created repo: $REPO_ID"

# Create git auth header for protocol tests
GIT_AUTH_HEADER=$(echo -n "x-token:$USER_TOKEN" | base64)

# Initialize git repo and push content
TEMP_GIT_DIR=$(mktemp -d)
cd "$TEMP_GIT_DIR"
git init -q
git config user.email "test@example.com"
git config user.name "Test User"

# Create README and some files
echo "# Test Repository" > README.md
echo "This is a test repository for API testing." >> README.md
mkdir -p src
echo 'fn main() { println!("Hello"); }' > src/main.rs

git add .
git commit -q -m "Initial commit"

# Create a second commit for diff/compare tests
echo "More content" >> README.md
git add .
git commit -q -m "Add more content"

# Push to server
git remote add origin "$BASE_URL/git/test-user/test-repo"
git -c http.extraHeader="Authorization: Basic $GIT_AUTH_HEADER" push -q origin main 2>/dev/null || true

# Get the commit SHA for tests
COMMIT_SHA=$(git rev-parse HEAD)

cd "$PROJECT_ROOT"
rm -rf "$TEMP_GIT_DIR"

echo "Pushed git content (HEAD: $COMMIT_SHA)"

# Generate unique suffix for test names
TEST_SUFFIX=$(date +%s)

# Write variables file
cat > "$VARS_FILE" << EOF
base_url=$BASE_URL
admin_token=$ADMIN_TOKEN
user_token=$USER_TOKEN
namespace_id=$NAMESPACE_ID
namespace_name=test-namespace
user_id=$USER_ID
user_ns_id=$USER_NS_ID
user_ns_name=test-user
repo_id=$REPO_ID
repo_name=test-repo
token_id=$TOKEN_ID
commit_sha=$COMMIT_SHA
git_auth_header=Basic $GIT_AUTH_HEADER
test_suffix=$TEST_SUFFIX
EOF

echo "Variables written to $VARS_FILE"

echo ""
echo "=== Running API tests ==="

# Collect all test files in order
TEST_DIR="$PROJECT_ROOT/tests/api"
TEST_FILES=(
    "$TEST_DIR/health.hurl"
    "$TEST_DIR/admin/grants.hurl"
    "$TEST_DIR/admin/namespaces.hurl"
    "$TEST_DIR/admin/tokens.hurl"
    "$TEST_DIR/admin/users.hurl"
    "$TEST_DIR/user/namespaces.hurl"
    "$TEST_DIR/user/repos.hurl"
    "$TEST_DIR/user/repo_tags.hurl"
    "$TEST_DIR/user/repo_folder.hurl"
    "$TEST_DIR/user/tags.hurl"
    "$TEST_DIR/user/folders.hurl"
    "$TEST_DIR/content/refs.hurl"
    "$TEST_DIR/content/commits.hurl"
    "$TEST_DIR/content/tree.hurl"
    "$TEST_DIR/content/blob.hurl"
    "$TEST_DIR/content/compare.hurl"
    "$TEST_DIR/content/blame.hurl"
    "$TEST_DIR/content/archive.hurl"
    "$TEST_DIR/content/readme.hurl"
    "$TEST_DIR/git/protocol.hurl"
)

# Run all tests sequentially with --jobs 1
if hurl --test --jobs 1 --variables-file "$VARS_FILE" "${TEST_FILES[@]}"; then
    echo ""
    echo "=== All tests passed ==="
    exit 0
else
    echo ""
    echo "=== Some tests failed ==="
    exit 1
fi
