#!/bin/bash
# Creates sample data for testing the TUI
# Uses credentials from ~/.config/cutman/credentials.toml

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Parse credentials file
CREDS_FILE="${HOME}/.config/cutman/credentials.toml"

if [ ! -f "$CREDS_FILE" ]; then
    echo -e "${RED}Error: Not logged in. Run 'cutman auth login' first.${NC}"
    exit 1
fi

# Extract server URL and token from TOML
SERVER_URL=$(grep 'server_url' "$CREDS_FILE" | sed 's/.*= *"\(.*\)"/\1/')
TOKEN=$(grep '^token' "$CREDS_FILE" | sed 's/.*= *"\(.*\)"/\1/')

if [ -z "$SERVER_URL" ] || [ -z "$TOKEN" ]; then
    echo -e "${RED}Error: Could not parse credentials from $CREDS_FILE${NC}"
    cat "$CREDS_FILE"
    exit 1
fi

API_URL="${SERVER_URL}/api/v1"

echo -e "${YELLOW}Using server: $SERVER_URL${NC}"

# Helper function to make API calls
api_call() {
    local method=$1
    local endpoint=$2
    local data=$3

    if [ -n "$data" ]; then
        curl -s -X "$method" \
            -H "Authorization: Bearer $TOKEN" \
            -H "Content-Type: application/json" \
            -d "$data" \
            "${API_URL}${endpoint}"
    else
        curl -s -X "$method" \
            -H "Authorization: Bearer $TOKEN" \
            "${API_URL}${endpoint}"
    fi
}

# Extract ID from JSON response using Python (more reliable than grep)
extract_id() {
    python3 -c "import sys, json; d=json.load(sys.stdin); print(d.get('data', {}).get('id', '') if isinstance(d.get('data'), dict) else '')"
}

# Extract repo ID by name from list response
get_repo_id_by_name() {
    local name=$1
    python3 -c "
import sys, json
data = json.load(sys.stdin)
repos = data.get('data', [])
for r in repos:
    if r.get('name') == '$name':
        print(r.get('id', ''))
        break
"
}

# Create a repo and return its ID
create_repo() {
    local name=$1
    local description=$2
    echo -e "${GREEN}Creating repo: $name${NC}" >&2
    local result=$(api_call POST "/repos" "{\"name\": \"$name\", \"description\": \"$description\"}")
    echo "$result" | extract_id
}

# Create a folder and return its ID
create_folder() {
    local name=$1
    local parent_id=$2
    echo -e "${GREEN}Creating folder: $name${NC}" >&2
    local data
    if [ -n "$parent_id" ]; then
        data="{\"name\": \"$name\", \"parent_id\": \"$parent_id\"}"
    else
        data="{\"name\": \"$name\"}"
    fi
    local result=$(api_call POST "/folders" "$data")
    echo "$result" | extract_id
}

# Move repo to folder
move_repo_to_folder() {
    local repo_id=$1
    local folder_id=$2
    echo -e "${GREEN}Moving repo $repo_id to folder $folder_id${NC}" >&2
    api_call POST "/repos/$repo_id/folders" "{\"folder_id\": \"$folder_id\"}" > /dev/null
}

# Create a tag
create_tag() {
    local name=$1
    local color=$2
    echo -e "${GREEN}Creating tag: $name${NC}" >&2
    api_call POST "/tags" "{\"name\": \"$name\", \"color\": \"$color\"}" > /dev/null
}

echo ""
echo "================================"
echo "Creating Sample Data for TUI"
echo "================================"
echo ""

# === Root-level repos (will appear under [Uncategorized]) ===
echo -e "\n${YELLOW}Creating root-level repos...${NC}"
create_repo "scratch" "Quick experiments" > /dev/null
create_repo "dotfiles" "Config files" > /dev/null
create_repo "notes" "Personal notes" > /dev/null

# === Create folder hierarchy ===
echo -e "\n${YELLOW}Creating folders...${NC}"

# Top-level folders
projects_id=$(create_folder "projects")
archive_id=$(create_folder "archive")
libs_id=$(create_folder "libs")

echo "  projects_id: $projects_id"
echo "  archive_id: $archive_id"
echo "  libs_id: $libs_id"

# Subfolders under projects
web_id=$(create_folder "web" "$projects_id")
mobile_id=$(create_folder "mobile" "$projects_id")
cli_id=$(create_folder "cli" "$projects_id")

echo "  web_id: $web_id"
echo "  mobile_id: $mobile_id"
echo "  cli_id: $cli_id"

# Subfolders under web
frontend_id=$(create_folder "frontend" "$web_id")
backend_id=$(create_folder "backend" "$web_id")

echo "  frontend_id: $frontend_id"
echo "  backend_id: $backend_id"

# Empty subfolders under mobile
ios_id=$(create_folder "ios" "$mobile_id")
android_id=$(create_folder "android" "$mobile_id")

# === Repos in folders (create and move in one step) ===
echo -e "\n${YELLOW}Creating repos in folders...${NC}"

# Repos for frontend folder
repo_id=$(create_repo "dashboard-ui" "Admin dashboard")
[ -n "$repo_id" ] && [ -n "$frontend_id" ] && move_repo_to_folder "$repo_id" "$frontend_id"

repo_id=$(create_repo "marketing-site" "Marketing website")
[ -n "$repo_id" ] && [ -n "$frontend_id" ] && move_repo_to_folder "$repo_id" "$frontend_id"

repo_id=$(create_repo "component-lib" "UI components")
[ -n "$repo_id" ] && [ -n "$frontend_id" ] && move_repo_to_folder "$repo_id" "$frontend_id"

# Repos for backend folder
repo_id=$(create_repo "api-server" "REST API server")
[ -n "$repo_id" ] && [ -n "$backend_id" ] && move_repo_to_folder "$repo_id" "$backend_id"

repo_id=$(create_repo "auth-service" "Auth microservice")
[ -n "$repo_id" ] && [ -n "$backend_id" ] && move_repo_to_folder "$repo_id" "$backend_id"

# Repos for cli folder
repo_id=$(create_repo "devtools" "Dev CLI tools")
[ -n "$repo_id" ] && [ -n "$cli_id" ] && move_repo_to_folder "$repo_id" "$cli_id"

# Repos for libs folder
repo_id=$(create_repo "utils" "Utility functions")
[ -n "$repo_id" ] && [ -n "$libs_id" ] && move_repo_to_folder "$repo_id" "$libs_id"

repo_id=$(create_repo "config-loader" "Config parser")
[ -n "$repo_id" ] && [ -n "$libs_id" ] && move_repo_to_folder "$repo_id" "$libs_id"

# Repos for archive folder
repo_id=$(create_repo "legacy-app" "Deprecated app")
[ -n "$repo_id" ] && [ -n "$archive_id" ] && move_repo_to_folder "$repo_id" "$archive_id"

repo_id=$(create_repo "old-website" "Old website")
[ -n "$repo_id" ] && [ -n "$archive_id" ] && move_repo_to_folder "$repo_id" "$archive_id"

# === Create tags ===
echo -e "\n${YELLOW}Creating tags...${NC}"
create_tag "active" "#22c55e"
create_tag "archived" "#6b7280"
create_tag "urgent" "#ef4444"
create_tag "docs" "#3b82f6"
create_tag "review" "#f59e0b"

echo ""
echo "================================"
echo -e "${GREEN}Sample data created!${NC}"
echo "================================"
echo ""
echo "Created:"
echo "  - 3 root-level repos (in [Uncategorized])"
echo "  - Folder hierarchy with nested folders"
echo "  - 10 repos in various folders"
echo "  - 2 empty folders (ios, android)"
echo "  - 5 tags"
echo ""
echo "Run 'cutman manage' to test the TUI"
