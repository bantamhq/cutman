# User Folders API Tests (Materialized Path)

# List folders - success (empty initially)
GET {{base_url}}/api/v1/folders
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# List folders with namespace filter
GET {{base_url}}/api/v1/folders?namespace={{user_ns_name}}
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Create root folder - success
POST {{base_url}}/api/v1/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "path": "/hurl-folder-{{test_suffix}}",
    "namespace": "{{user_ns_name}}"
}
HTTP 201
[Asserts]
jsonpath "$.data.id" isInteger
jsonpath "$.data.path" == "/hurl-folder-{{test_suffix}}"
[Captures]
root_folder_id: jsonpath "$.data.id"

# Create nested folder - success (auto-creates parent if needed)
POST {{base_url}}/api/v1/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "path": "/hurl-folder-{{test_suffix}}/child",
    "namespace": "{{user_ns_name}}"
}
HTTP 201
[Asserts]
jsonpath "$.data.id" isInteger
jsonpath "$.data.path" == "/hurl-folder-{{test_suffix}}/child"
[Captures]
child_folder_id: jsonpath "$.data.id"

# Create folder - conflict (duplicate path)
POST {{base_url}}/api/v1/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "path": "/hurl-folder-{{test_suffix}}/child",
    "namespace": "{{user_ns_name}}"
}
HTTP 409
[Asserts]
jsonpath "$.error" contains "already exists"

# Get specific folder - success
GET {{base_url}}/api/v1/folders/{{child_folder_id}}
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.id" == {{child_folder_id}}
jsonpath "$.data.path" == "/hurl-folder-{{test_suffix}}/child"

# Get folder - not found
GET {{base_url}}/api/v1/folders/99999999
Authorization: Bearer {{user_token}}
HTTP 404
[Asserts]
jsonpath "$.error" contains "not found"

# List repos in folder (empty)
GET {{base_url}}/api/v1/folders/{{root_folder_id}}/repos
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Update folder - move/rename to new path
PATCH {{base_url}}/api/v1/folders/{{child_folder_id}}
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "path": "/hurl-folder-{{test_suffix}}/renamed"
}
HTTP 200
[Asserts]
jsonpath "$.data.path" == "/hurl-folder-{{test_suffix}}/renamed"

# Create another root folder to test move across trees
POST {{base_url}}/api/v1/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "path": "/hurl-folder-other-{{test_suffix}}",
    "namespace": "{{user_ns_name}}"
}
HTTP 201
[Captures]
other_root_id: jsonpath "$.data.id"

# Move folder to different tree
PATCH {{base_url}}/api/v1/folders/{{child_folder_id}}
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "path": "/hurl-folder-other-{{test_suffix}}/moved"
}
HTTP 200
[Asserts]
jsonpath "$.data.path" == "/hurl-folder-other-{{test_suffix}}/moved"

# Delete folder - success (cascades, moves repos to root)
DELETE {{base_url}}/api/v1/folders/{{child_folder_id}}
Authorization: Bearer {{user_token}}
HTTP 204

# Delete folder - not found
DELETE {{base_url}}/api/v1/folders/99999999
Authorization: Bearer {{user_token}}
HTTP 404

# Clean up remaining folders
DELETE {{base_url}}/api/v1/folders/{{other_root_id}}
Authorization: Bearer {{user_token}}
HTTP 204

DELETE {{base_url}}/api/v1/folders/{{root_folder_id}}
Authorization: Bearer {{user_token}}
HTTP 204

# Folders - unauthorized (no token)
GET {{base_url}}/api/v1/folders
HTTP 401
