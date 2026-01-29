# Content Mutation API Tests

# ============================================================================
# PUT Blob - Create new file
# ============================================================================

# Create a new file - success
PUT {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/new-file.md
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "message": "Create new file",
    "content": "# New File\n\nThis is a new file."
}
HTTP 201
[Asserts]
jsonpath "$.data.commit_sha" isString
jsonpath "$.data.ref_name" == "main"
jsonpath "$.data.file.path" == "new-file.md"
jsonpath "$.data.file.sha" isString
jsonpath "$.data.file.size" isInteger
[Captures]
new_file_sha: jsonpath "$.data.file.sha"

# Create file that already exists - conflict without sha
PUT {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/new-file.md
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "message": "Create duplicate file",
    "content": "# Duplicate\n\nThis should fail."
}
HTTP 409
[Asserts]
jsonpath "$.error" contains "already exists"

# Update file with correct sha - success
PUT {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/new-file.md
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "message": "Update existing file",
    "content": "# Updated File\n\nThis file has been updated.",
    "sha": "{{new_file_sha}}"
}
HTTP 201
[Asserts]
jsonpath "$.data.commit_sha" isString
jsonpath "$.data.file.path" == "new-file.md"
[Captures]
updated_file_sha: jsonpath "$.data.file.sha"

# Update file with wrong sha - conflict
PUT {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/new-file.md
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "message": "Update with wrong sha",
    "content": "# Wrong SHA\n\nThis should fail.",
    "sha": "0000000000000000000000000000000000000000"
}
HTTP 409
[Asserts]
jsonpath "$.error" contains "modified"

# Create nested file - success
PUT {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/docs/guide/intro.md
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "message": "Create nested file",
    "content": "# Introduction\n\nWelcome to the guide."
}
HTTP 201
[Asserts]
jsonpath "$.data.file.path" == "docs/guide/intro.md"

# Create file with base64 encoding - success
PUT {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/binary-test.bin
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "message": "Create binary file",
    "content": "SGVsbG8gV29ybGQh",
    "encoding": "base64"
}
HTTP 201
[Asserts]
jsonpath "$.data.file.path" == "binary-test.bin"
[Captures]
binary_file_sha: jsonpath "$.data.file.sha"

# PUT blob - unauthorized
PUT {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/unauthorized.md
Content-Type: application/json
{
    "message": "Unauthorized create",
    "content": "This should fail."
}
HTTP 401

# ============================================================================
# DELETE Blob
# ============================================================================

# Delete file - success
DELETE {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/binary-test.bin
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "message": "Delete binary file",
    "sha": "{{binary_file_sha}}"
}
HTTP 200
[Asserts]
jsonpath "$.data.commit_sha" isString
jsonpath "$.data.ref_name" == "main"

# Delete file with wrong sha - conflict
DELETE {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/new-file.md
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "message": "Delete with wrong sha",
    "sha": "0000000000000000000000000000000000000000"
}
HTTP 409

# Delete nonexistent file - not found
DELETE {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/nonexistent.md
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "message": "Delete nonexistent",
    "sha": "0000000000000000000000000000000000000000"
}
HTTP 404

# Delete blob - unauthorized
DELETE {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/new-file.md
Content-Type: application/json
{
    "message": "Unauthorized delete",
    "sha": "{{updated_file_sha}}"
}
HTTP 401

# ============================================================================
# Multi-file Commit
# ============================================================================

# Multi-file commit - success
POST {{base_url}}/api/v1/repos/{{repo_id}}/commits
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "message": "Multi-file commit",
    "actions": [
        {
            "action": "create",
            "path": "multi/file1.md",
            "content": "# File 1"
        },
        {
            "action": "create",
            "path": "multi/file2.md",
            "content": "# File 2"
        }
    ]
}
HTTP 201
[Asserts]
jsonpath "$.data.commit_sha" isString
jsonpath "$.data.ref_name" == "main"

# Multi-file commit with update - success
POST {{base_url}}/api/v1/repos/{{repo_id}}/commits
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "message": "Update and create",
    "actions": [
        {
            "action": "update",
            "path": "multi/file1.md",
            "content": "# File 1 Updated"
        },
        {
            "action": "create",
            "path": "multi/file3.md",
            "content": "# File 3"
        }
    ]
}
HTTP 201

# Multi-file commit - empty actions
POST {{base_url}}/api/v1/repos/{{repo_id}}/commits
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "message": "Empty commit",
    "actions": []
}
HTTP 400
[Asserts]
jsonpath "$.error" contains "action"

# Multi-file commit - unauthorized
POST {{base_url}}/api/v1/repos/{{repo_id}}/commits
Content-Type: application/json
{
    "message": "Unauthorized commit",
    "actions": [
        {
            "action": "create",
            "path": "unauthorized.md",
            "content": "# Unauthorized"
        }
    ]
}
HTTP 401

# ============================================================================
# Enhanced Blob (history and parsed)
# ============================================================================

# Get blob with history
GET {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/README.md?history=true
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.sha" isString
jsonpath "$.data.history" isCollection
jsonpath "$.data.history_has_more" isBoolean

# Get blob with parsed frontmatter
PUT {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/with-frontmatter.md
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "message": "Create file with frontmatter",
    "content": "---\ntitle: Test Post\nauthor: John Doe\ntags:\n  - test\n  - api\n---\n\n# Test Post\n\nThis is the body content."
}
HTTP 201

GET {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/with-frontmatter.md?parsed=true
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.sha" isString
jsonpath "$.data.frontmatter.title" == "Test Post"
jsonpath "$.data.frontmatter.author" == "John Doe"
jsonpath "$.data.frontmatter.tags[0]" == "test"
jsonpath "$.data.body" contains "Test Post"

# Get blob with both history and parsed
GET {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/with-frontmatter.md?history=true&parsed=true
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.frontmatter" exists
jsonpath "$.data.body" exists
jsonpath "$.data.history" isCollection

# ============================================================================
# Path Search
# ============================================================================

# Search paths - success
GET {{base_url}}/api/v1/repos/{{repo_id}}/search?q=*.md
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.matches" isCollection

# Search paths - with ref
GET {{base_url}}/api/v1/repos/{{repo_id}}/search?q=src/*&ref=main
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.matches" isCollection

# Search paths - with limit
GET {{base_url}}/api/v1/repos/{{repo_id}}/search?q=*&limit=5
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.matches" isCollection

# Search paths - unauthorized
GET {{base_url}}/api/v1/repos/{{repo_id}}/search?q=*.md
HTTP 401
