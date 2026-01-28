# User Folders API Tests

# List folders - success
GET {{base_url}}/api/v1/folders
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.has_more" isBoolean

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
    "name": "hurl-folder-{{test_suffix}}",
    "namespace": "{{user_ns_name}}"
}
HTTP 201
[Asserts]
jsonpath "$.data.id" isString
jsonpath "$.data.name" == "hurl-folder-{{test_suffix}}"
jsonpath "$.data.parent_id" not exists
[Captures]
root_folder_id: jsonpath "$.data.id"

# Create nested folder - success
POST {{base_url}}/api/v1/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "hurl-folder-child-{{test_suffix}}",
    "namespace": "{{user_ns_name}}",
    "parent_id": "{{root_folder_id}}"
}
HTTP 201
[Asserts]
jsonpath "$.data.id" isString
jsonpath "$.data.name" == "hurl-folder-child-{{test_suffix}}"
jsonpath "$.data.parent_id" == {{root_folder_id}}
[Captures]
child_folder_id: jsonpath "$.data.id"

# Create folder - conflict (duplicate name in same parent)
POST {{base_url}}/api/v1/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "hurl-folder-child-{{test_suffix}}",
    "namespace": "{{user_ns_name}}",
    "parent_id": "{{root_folder_id}}"
}
HTTP 409
[Asserts]
jsonpath "$.error" contains "already exists"

# Create folder - with empty name (API allows this)
POST {{base_url}}/api/v1/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "",
    "namespace": "{{user_ns_name}}"
}
HTTP 201
[Captures]
empty_name_folder_id: jsonpath "$.data.id"

# Get specific folder - success (with full path)
GET {{base_url}}/api/v1/folders/{{child_folder_id}}
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.id" == {{child_folder_id}}
jsonpath "$.data.name" == "hurl-folder-child-{{test_suffix}}"
jsonpath "$.data.path" isString

# Get folder - not found
GET {{base_url}}/api/v1/folders/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{user_token}}
HTTP 404
[Asserts]
jsonpath "$.error" contains "not found"

# List children of folder
GET {{base_url}}/api/v1/folders/{{root_folder_id}}/children
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# List repos in folder (empty)
GET {{base_url}}/api/v1/folders/{{root_folder_id}}/repos
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Update folder - rename
PATCH {{base_url}}/api/v1/folders/{{child_folder_id}}
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "hurl-folder-renamed-{{test_suffix}}"
}
HTTP 200
[Asserts]
jsonpath "$.data.name" == "hurl-folder-renamed-{{test_suffix}}"

# Create another root folder to test move
POST {{base_url}}/api/v1/folders
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "hurl-folder-other-{{test_suffix}}",
    "namespace": "{{user_ns_name}}"
}
HTTP 201
[Captures]
other_root_id: jsonpath "$.data.id"

# Update folder - move to different parent
PATCH {{base_url}}/api/v1/folders/{{child_folder_id}}
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "parent_id": "{{other_root_id}}"
}
HTTP 200
[Asserts]
jsonpath "$.data.parent_id" == {{other_root_id}}

# Delete folder - success
DELETE {{base_url}}/api/v1/folders/{{child_folder_id}}
Authorization: Bearer {{user_token}}
HTTP 204

# Delete folder - not found
DELETE {{base_url}}/api/v1/folders/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{user_token}}
HTTP 404

# Clean up remaining folders
DELETE {{base_url}}/api/v1/folders/{{other_root_id}}
Authorization: Bearer {{user_token}}
HTTP 204

DELETE {{base_url}}/api/v1/folders/{{root_folder_id}}
Authorization: Bearer {{user_token}}
HTTP 204

# Clean up empty name folder
DELETE {{base_url}}/api/v1/folders/{{empty_name_folder_id}}
Authorization: Bearer {{user_token}}
HTTP 204

# Folders - unauthorized (no token)
GET {{base_url}}/api/v1/folders
HTTP 401
