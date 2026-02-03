# Admin Token API Tests

# List all tokens - success
GET {{base_url}}/api/v1/admin/tokens
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.has_more" isBoolean

# Get specific token - success
GET {{base_url}}/api/v1/admin/tokens/{{token_id}}
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data.id" == {{token_id}}

# Get token - not found
GET {{base_url}}/api/v1/admin/tokens/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{admin_token}}
HTTP 404
[Asserts]
jsonpath "$.error" contains "not found"

# Create a token to delete
POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "description": "Token to delete"
}
HTTP 201
[Captures]
delete_token_id: jsonpath "$.data.metadata.id"

# Delete token - success
DELETE {{base_url}}/api/v1/admin/tokens/{{delete_token_id}}
Authorization: Bearer {{admin_token}}
HTTP 204

# Delete token - not found
DELETE {{base_url}}/api/v1/admin/tokens/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{admin_token}}
HTTP 404

# List tokens - unauthorized (no token)
GET {{base_url}}/api/v1/admin/tokens
HTTP 401

# List tokens - forbidden (user token)
GET {{base_url}}/api/v1/admin/tokens
Authorization: Bearer {{principal_token}}
HTTP 403
