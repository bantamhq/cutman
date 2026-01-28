# Repository Folder API Tests

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

# Get folder for repo - initially null
GET {{base_url}}/api/v1/repos/{{repo_id}}/folder
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" == null

# Set folder for repo - success
PUT {{base_url}}/api/v1/repos/{{repo_id}}/folder
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "folder_id": "{{test_folder_id}}"
}
HTTP 200
[Asserts]
jsonpath "$.data.id" == {{test_folder_id}}

# Get folder for repo - should return folder
GET {{base_url}}/api/v1/repos/{{repo_id}}/folder
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.id" == {{test_folder_id}}
jsonpath "$.data.name" == "repo-folder-test"

# Clear folder for repo - success
PUT {{base_url}}/api/v1/repos/{{repo_id}}/folder
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "folder_id": null
}
HTTP 200
[Asserts]
jsonpath "$.data" == null

# Repo folder - unauthorized (no token)
GET {{base_url}}/api/v1/repos/{{repo_id}}/folder
HTTP 401

# Clean up: Delete test folder
DELETE {{base_url}}/api/v1/folders/{{test_folder_id}}
Authorization: Bearer {{user_token}}
HTTP 204
