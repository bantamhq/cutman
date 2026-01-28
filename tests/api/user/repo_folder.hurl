# Repository Folders API Tests

# Create a folder for testing
POST {{base_url}}/api/v1/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "repo-folder-test",
    "namespace": "{{user_ns_name}}"
}
HTTP 201
[Captures]
test_folder_id: jsonpath "$.data.id"

# Get folders for repo - initially empty array
GET {{base_url}}/api/v1/repos/{{repo_id}}/folders
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" count == 0

# Set folder for repo - success
POST {{base_url}}/api/v1/repos/{{repo_id}}/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "folder_id": "{{test_folder_id}}"
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
jsonpath "$.data[0].name" == "repo-folder-test"

# Clear folder for repo - wrong folder_id should fail
DELETE {{base_url}}/api/v1/repos/{{repo_id}}/folders/wrong-id
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

# Repo folders - unauthorized (no token)
GET {{base_url}}/api/v1/repos/{{repo_id}}/folders
HTTP 401

# Clean up: Delete test folder
DELETE {{base_url}}/api/v1/folders/{{test_folder_id}}
Authorization: Bearer {{user_token}}
HTTP 204
