# Admin Grants API Tests

# Create a test user for grant testing
POST {{base_url}}/api/v1/admin/users
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "namespace_name": "hurl-grants-{{test_suffix}}"
}
HTTP 201
[Captures]
grant_user_id: jsonpath "$.data.id"

# Create namespace grant - success
POST {{base_url}}/api/v1/admin/users/{{grant_user_id}}/namespace-grants
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "namespace_id": "{{namespace_id}}",
    "allow": ["namespace:read", "repo:read", "repo:write"]
}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# List namespace grants - success
GET {{base_url}}/api/v1/admin/users/{{grant_user_id}}/namespace-grants
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Get specific namespace grant - success
GET {{base_url}}/api/v1/admin/users/{{grant_user_id}}/namespace-grants/{{namespace_id}}
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data.namespace_id" == {{namespace_id}}
jsonpath "$.data.allow" isCollection

# Get namespace grant - not found
GET {{base_url}}/api/v1/admin/users/{{grant_user_id}}/namespace-grants/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{admin_token}}
HTTP 404

# Create namespace grant - invalid permissions
POST {{base_url}}/api/v1/admin/users/{{grant_user_id}}/namespace-grants
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "namespace_id": "{{namespace_id}}",
    "allow": ["invalid:permission"]
}
HTTP 400

# Create repo grant - success
POST {{base_url}}/api/v1/admin/users/{{grant_user_id}}/repo-grants
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "repo_id": "{{repo_id}}",
    "allow": ["repo:read", "repo:write"]
}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# List repo grants - success
GET {{base_url}}/api/v1/admin/users/{{grant_user_id}}/repo-grants
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Get specific repo grant - success
GET {{base_url}}/api/v1/admin/users/{{grant_user_id}}/repo-grants/{{repo_id}}
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data.repo_id" == {{repo_id}}
jsonpath "$.data.allow" isCollection

# Get repo grant - not found
GET {{base_url}}/api/v1/admin/users/{{grant_user_id}}/repo-grants/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{admin_token}}
HTTP 404

# Delete repo grant - success
DELETE {{base_url}}/api/v1/admin/users/{{grant_user_id}}/repo-grants/{{repo_id}}
Authorization: Bearer {{admin_token}}
HTTP 204

# Delete namespace grant - success
DELETE {{base_url}}/api/v1/admin/users/{{grant_user_id}}/namespace-grants/{{namespace_id}}
Authorization: Bearer {{admin_token}}
HTTP 204

# Grants - unauthorized (no token)
GET {{base_url}}/api/v1/admin/users/{{grant_user_id}}/namespace-grants
HTTP 401

# Grants - forbidden (user token)
GET {{base_url}}/api/v1/admin/users/{{grant_user_id}}/namespace-grants
Authorization: Bearer {{user_token}}
HTTP 403

# Clean up test user
DELETE {{base_url}}/api/v1/admin/users/{{grant_user_id}}
Authorization: Bearer {{admin_token}}
HTTP 204
