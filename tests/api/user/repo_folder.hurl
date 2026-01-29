# Repository Folders API Tests (Path-based)

# Create a folder for testing using path
POST {{base_url}}/api/v1/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "path": "/repo-folder-test-{{test_suffix}}",
    "namespace": "{{user_ns_name}}"
}
HTTP 201
[Captures]
test_folder_id: jsonpath "$.data.id"
test_folder_path: jsonpath "$.data.path"

# Get folders for repo - initially empty array
GET {{base_url}}/api/v1/repos/{{repo_id}}/folders
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" count == 0

# Set folder for repo by path - success (auto-creates folders)
POST {{base_url}}/api/v1/repos/{{repo_id}}/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "folder_path": "/repo-folder-test-{{test_suffix}}"
}
HTTP 200
[Asserts]
jsonpath "$.data" count == 1
jsonpath "$.data[0].id" == {{test_folder_id}}

# Get folders for repo - should return array with one folder
GET {{base_url}}/api/v1/repos/{{repo_id}}/folders
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" count == 1
jsonpath "$.data[0].id" == {{test_folder_id}}
jsonpath "$.data[0].path" == "/repo-folder-test-{{test_suffix}}"

# Clear folder for repo - wrong folder_id should fail
DELETE {{base_url}}/api/v1/repos/{{repo_id}}/folders/99999999
Authorization: Bearer {{user_token}}
HTTP 400

# Clear folder for repo - success
DELETE {{base_url}}/api/v1/repos/{{repo_id}}/folders/{{test_folder_id}}
Authorization: Bearer {{user_token}}
HTTP 204

# Get folders for repo - should be empty again
GET {{base_url}}/api/v1/repos/{{repo_id}}/folders
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" count == 0

# Set folder by path that doesn't exist yet (auto-creates)
POST {{base_url}}/api/v1/repos/{{repo_id}}/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "folder_path": "/auto-created-{{test_suffix}}/nested"
}
HTTP 200
[Asserts]
jsonpath "$.data" count == 1
jsonpath "$.data[0].path" == "/auto-created-{{test_suffix}}/nested"
[Captures]
auto_folder_id: jsonpath "$.data[0].id"

# Clear folder to move to root
POST {{base_url}}/api/v1/repos/{{repo_id}}/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "folder_path": null
}
HTTP 200
[Asserts]
jsonpath "$.data" count == 0

# Repo folders - unauthorized (no token)
GET {{base_url}}/api/v1/repos/{{repo_id}}/folders
HTTP 401

# Clean up: Delete test folders
DELETE {{base_url}}/api/v1/folders/{{test_folder_id}}
Authorization: Bearer {{user_token}}
HTTP 204

DELETE {{base_url}}/api/v1/folders/{{auto_folder_id}}
Authorization: Bearer {{user_token}}
HTTP 204
