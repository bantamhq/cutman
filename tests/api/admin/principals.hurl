# Admin Principal API Tests

# List principals - success
GET {{base_url}}/api/v1/admin/principals
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.has_more" isBoolean

# Get specific principal - success
GET {{base_url}}/api/v1/admin/principals/{{principal_id}}
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data.id" == {{principal_id}}
jsonpath "$.data.primary_namespace_id" == {{principal_ns_id}}

# Get principal - not found
GET {{base_url}}/api/v1/admin/principals/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{admin_token}}
HTTP 404
[Asserts]
jsonpath "$.error" contains "not found"

# Create principal - success
POST {{base_url}}/api/v1/admin/principals
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "namespace_name": "hurl-principals-{{test_suffix}}"
}
HTTP 201
[Asserts]
jsonpath "$.data.id" isString
jsonpath "$.data.primary_namespace_id" isString
[Captures]
new_principal_id: jsonpath "$.data.id"
new_principal_ns_id: jsonpath "$.data.primary_namespace_id"

# Create principal - conflict (duplicate namespace name)
POST {{base_url}}/api/v1/admin/principals
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "namespace_name": "hurl-principals-{{test_suffix}}"
}
HTTP 409
[Asserts]
jsonpath "$.error" contains "already exists"

# Create principal - validation error (empty namespace name)
POST {{base_url}}/api/v1/admin/principals
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "namespace_name": ""
}
HTTP 400
[Asserts]
jsonpath "$.error" contains "empty"

# List principal tokens - success
GET {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Create principal token - success (no expiry)
POST {{base_url}}/api/v1/admin/principals/{{new_principal_id}}/tokens
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
new_principal_token: jsonpath "$.data.token"

# Create principal token - success (with expiry param)
POST {{base_url}}/api/v1/admin/principals/{{new_principal_id}}/tokens
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

# List principals - unauthorized (no token)
GET {{base_url}}/api/v1/admin/principals
HTTP 401

# List principals - forbidden (principal token instead of admin)
GET {{base_url}}/api/v1/admin/principals
Authorization: Bearer {{principal_token}}
HTTP 403

# Delete principal - success
DELETE {{base_url}}/api/v1/admin/principals/{{new_principal_id}}
Authorization: Bearer {{admin_token}}
HTTP 204

# Delete principal - not found
DELETE {{base_url}}/api/v1/admin/principals/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{admin_token}}
HTTP 404
