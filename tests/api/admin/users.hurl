# Admin User API Tests

# List users - success
GET {{base_url}}/api/v1/admin/users
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.has_more" isBoolean

# Get specific user - success
GET {{base_url}}/api/v1/admin/users/{{user_id}}
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data.id" == {{user_id}}
jsonpath "$.data.primary_namespace_id" == {{user_ns_id}}

# Get user - not found
GET {{base_url}}/api/v1/admin/users/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{admin_token}}
HTTP 404
[Asserts]
jsonpath "$.error" contains "not found"

# Create user - success
POST {{base_url}}/api/v1/admin/users
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "namespace_name": "hurl-users-{{test_suffix}}"
}
HTTP 201
[Asserts]
jsonpath "$.data.id" isString
jsonpath "$.data.primary_namespace_id" isString
[Captures]
new_user_id: jsonpath "$.data.id"
new_user_ns_id: jsonpath "$.data.primary_namespace_id"

# Create user - conflict (duplicate namespace name)
POST {{base_url}}/api/v1/admin/users
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "namespace_name": "hurl-users-{{test_suffix}}"
}
HTTP 409
[Asserts]
jsonpath "$.error" contains "already exists"

# Create user - validation error (empty namespace name)
POST {{base_url}}/api/v1/admin/users
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "namespace_name": ""
}
HTTP 400
[Asserts]
jsonpath "$.error" contains "empty"

# List user tokens - success
GET {{base_url}}/api/v1/admin/users/{{user_id}}/tokens
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Create user token - success (no expiry)
POST {{base_url}}/api/v1/admin/users/{{new_user_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "description": "Test token"
}
HTTP 201
[Asserts]
jsonpath "$.data.token" isString
jsonpath "$.data.metadata.id" isString
[Captures]
new_token_id: jsonpath "$.data.metadata.id"
new_user_token: jsonpath "$.data.token"

# Create user token - success (with expiry param)
POST {{base_url}}/api/v1/admin/users/{{new_user_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "description": "Expiring token",
    "expires_in_days": 30
}
HTTP 201
[Asserts]
jsonpath "$.data.token" isString
jsonpath "$.data.metadata.id" isString

# List users - unauthorized (no token)
GET {{base_url}}/api/v1/admin/users
HTTP 401

# List users - forbidden (user token instead of admin)
GET {{base_url}}/api/v1/admin/users
Authorization: Bearer {{user_token}}
HTTP 403

# Delete user - success
DELETE {{base_url}}/api/v1/admin/users/{{new_user_id}}
Authorization: Bearer {{admin_token}}
HTTP 204

# Delete user - not found
DELETE {{base_url}}/api/v1/admin/users/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{admin_token}}
HTTP 404
