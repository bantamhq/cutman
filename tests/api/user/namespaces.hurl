# User Namespace API Tests

# List accessible namespaces - success
GET {{base_url}}/api/v1/namespaces
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# List namespace grants - success
GET {{base_url}}/api/v1/namespaces/{{user_ns_name}}/grants
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Create a secondary namespace for update/delete test
POST {{base_url}}/api/v1/admin/namespaces
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "name": "hurl-ns-{{test_suffix}}"
}
HTTP 201
[Captures]
deletable_ns_id: jsonpath "$.data.id"

# Grant user admin access to the namespace
POST {{base_url}}/api/v1/admin/users/{{user_id}}/namespace-grants
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "namespace_id": "{{deletable_ns_id}}",
    "allow": ["namespace:read", "namespace:write", "namespace:admin"]
}
HTTP 200

# Update namespace - success (requires namespace:admin)
PATCH {{base_url}}/api/v1/namespaces/hurl-ns-{{test_suffix}}
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "repo_limit": 200
}
HTTP 200
[Asserts]
jsonpath "$.data.repo_limit" == 200

# Delete namespace - success (non-primary)
DELETE {{base_url}}/api/v1/namespaces/hurl-ns-{{test_suffix}}
Authorization: Bearer {{user_token}}
HTTP 204

# Namespaces - unauthorized (no token)
GET {{base_url}}/api/v1/namespaces
HTTP 401
