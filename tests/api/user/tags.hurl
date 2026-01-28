# User Tags API Tests

# List tags - success
GET {{base_url}}/api/v1/tags
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.has_more" isBoolean

# List tags with namespace filter
GET {{base_url}}/api/v1/tags?namespace={{user_ns_name}}
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Create tag - success
POST {{base_url}}/api/v1/tags
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "hurl-tag-{{test_suffix}}",
    "color": "#3498DB",
    "namespace": "{{user_ns_name}}"
}
HTTP 201
[Asserts]
jsonpath "$.data.id" isString
jsonpath "$.data.name" == "hurl-tag-{{test_suffix}}"
jsonpath "$.data.color" == "#3498DB"
[Captures]
new_tag_id: jsonpath "$.data.id"

# Create tag - conflict (duplicate name in same namespace)
POST {{base_url}}/api/v1/tags
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "hurl-tag-{{test_suffix}}",
    "namespace": "{{user_ns_name}}"
}
HTTP 409
[Asserts]
jsonpath "$.error" contains "already exists"

# Create tag - with empty name (API rejects this)
POST {{base_url}}/api/v1/tags
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "",
    "namespace": "{{user_ns_name}}"
}
HTTP 400
[Asserts]
jsonpath "$.error" contains "cannot be empty"

# Get specific tag - success
GET {{base_url}}/api/v1/tags/{{new_tag_id}}
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.id" == {{new_tag_id}}
jsonpath "$.data.name" == "hurl-tag-{{test_suffix}}"

# Get tag - not found
GET {{base_url}}/api/v1/tags/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{user_token}}
HTTP 404
[Asserts]
jsonpath "$.error" contains "not found"

# Update tag - success
PATCH {{base_url}}/api/v1/tags/{{new_tag_id}}
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "hurl-tag-renamed-{{test_suffix}}",
    "color": "#E74C3C"
}
HTTP 200
[Asserts]
jsonpath "$.data.name" == "hurl-tag-renamed-{{test_suffix}}"
jsonpath "$.data.color" == "#E74C3C"

# Delete tag - success (with force)
DELETE {{base_url}}/api/v1/tags/{{new_tag_id}}?force=true
Authorization: Bearer {{user_token}}
HTTP 204

# Delete tag - not found
DELETE {{base_url}}/api/v1/tags/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{user_token}}
HTTP 404

# Tags - unauthorized (no token)
GET {{base_url}}/api/v1/tags
HTTP 401
