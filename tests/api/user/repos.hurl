# User Repository API Tests

# List repos - success
GET {{base_url}}/api/v1/repos
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.has_more" isBoolean

# List repos with namespace filter
GET {{base_url}}/api/v1/repos?namespace={{user_ns_name}}
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Get specific repo - success
GET {{base_url}}/api/v1/repos/{{repo_id}}
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.id" == {{repo_id}}
jsonpath "$.data.name" == "test-repo"
jsonpath "$.data.description" == "Test repository"

# Get repo - not found
GET {{base_url}}/api/v1/repos/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{user_token}}
HTTP 404
[Asserts]
jsonpath "$.error" contains "not found"

# Create repo - success
POST {{base_url}}/api/v1/repos
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "hurl-repo-{{test_suffix}}",
    "description": "A test repository",
    "public": true,
    "namespace": "{{user_ns_name}}"
}
HTTP 201
[Asserts]
jsonpath "$.data.id" isString
jsonpath "$.data.name" == "hurl-repo-{{test_suffix}}"
jsonpath "$.data.description" == "A test repository"
jsonpath "$.data.public" == true
[Captures]
new_repo_id: jsonpath "$.data.id"

# Create repo - conflict (duplicate name in same namespace)
POST {{base_url}}/api/v1/repos
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "hurl-repo-{{test_suffix}}",
    "namespace": "{{user_ns_name}}"
}
HTTP 409
[Asserts]
jsonpath "$.error" contains "already exists"

# Create repo - with empty name (API allows this)
POST {{base_url}}/api/v1/repos
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "",
    "namespace": "{{user_ns_name}}"
}
HTTP 201
[Captures]
empty_name_repo_id: jsonpath "$.data.id"

# Update repo - success
PATCH {{base_url}}/api/v1/repos/{{new_repo_id}}
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "description": "Updated description",
    "public": false
}
HTTP 200
[Asserts]
jsonpath "$.data.description" == "Updated description"
jsonpath "$.data.public" == false

# Update repo - rename
PATCH {{base_url}}/api/v1/repos/{{new_repo_id}}
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "hurl-repo-renamed-{{test_suffix}}"
}
HTTP 200
[Asserts]
jsonpath "$.data.name" == "hurl-repo-renamed-{{test_suffix}}"

# Delete repo - success
DELETE {{base_url}}/api/v1/repos/{{new_repo_id}}
Authorization: Bearer {{user_token}}
HTTP 204

# Delete repo - not found
DELETE {{base_url}}/api/v1/repos/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{user_token}}
HTTP 404

# Clean up empty name repo
DELETE {{base_url}}/api/v1/repos/{{empty_name_repo_id}}
Authorization: Bearer {{user_token}}
HTTP 204

# Repos - unauthorized (no token)
GET {{base_url}}/api/v1/repos
HTTP 401
