# Admin Namespace API Tests

# List namespaces - success
GET {{base_url}}/api/v1/admin/namespaces
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.has_more" isBoolean

# Get specific namespace - success
GET {{base_url}}/api/v1/admin/namespaces/{{namespace_name}}
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data.id" == {{namespace_id}}
jsonpath "$.data.name" == "test-namespace"

# Get namespace - not found
GET {{base_url}}/api/v1/admin/namespaces/nonexistent-namespace
Authorization: Bearer {{admin_token}}
HTTP 404
[Asserts]
jsonpath "$.error" contains "not found"

# Create namespace - success
POST {{base_url}}/api/v1/admin/namespaces
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "name": "new-test-namespace",
    "repo_limit": 50,
    "storage_limit_bytes": 1000000
}
HTTP 201
[Asserts]
jsonpath "$.data.id" isString
jsonpath "$.data.name" == "new-test-namespace"
jsonpath "$.data.repo_limit" == 50
[Captures]
new_namespace_id: jsonpath "$.data.id"

# Create namespace - conflict (duplicate name)
POST {{base_url}}/api/v1/admin/namespaces
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "name": "new-test-namespace"
}
HTTP 409
[Asserts]
jsonpath "$.error" contains "already exists"

# Create namespace - validation error (empty name)
POST {{base_url}}/api/v1/admin/namespaces
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "name": ""
}
HTTP 400
[Asserts]
jsonpath "$.error" isString

# Create namespace - validation error (invalid characters)
POST {{base_url}}/api/v1/admin/namespaces
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "name": "Invalid Name!"
}
HTTP 400
[Asserts]
jsonpath "$.error" isString

# List namespaces - unauthorized (no token)
GET {{base_url}}/api/v1/admin/namespaces
HTTP 401

# List namespaces - forbidden (user token instead of admin)
GET {{base_url}}/api/v1/admin/namespaces
Authorization: Bearer {{user_token}}
HTTP 403

# Delete namespace - success (the one we just created)
DELETE {{base_url}}/api/v1/admin/namespaces/new-test-namespace
Authorization: Bearer {{admin_token}}
HTTP 204

# Delete namespace - not found
DELETE {{base_url}}/api/v1/admin/namespaces/nonexistent-namespace
Authorization: Bearer {{admin_token}}
HTTP 404
