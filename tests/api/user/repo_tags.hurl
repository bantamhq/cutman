# Repository Tags API Tests

# Create a tag for testing
POST {{base_url}}/api/v1/tags
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "repo-tag-test",
    "color": "#FF0000",
    "namespace": "{{user_ns_name}}"
}
HTTP 201
[Captures]
test_tag_id: jsonpath "$.data.id"

# Create another tag
POST {{base_url}}/api/v1/tags
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "name": "repo-tag-test-2",
    "color": "#00FF00",
    "namespace": "{{user_ns_name}}"
}
HTTP 201
[Captures]
test_tag_id_2: jsonpath "$.data.id"

# List tags on repo - initially empty
GET {{base_url}}/api/v1/repos/{{repo_id}}/tags
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Add tags to repo - success
POST {{base_url}}/api/v1/repos/{{repo_id}}/tags
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "tag_ids": ["{{test_tag_id}}"]
}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# List tags on repo - should have one tag
GET {{base_url}}/api/v1/repos/{{repo_id}}/tags
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Replace all tags on repo - success
PUT {{base_url}}/api/v1/repos/{{repo_id}}/tags
Authorization: Bearer {{user_token}}
Content-Type: application/json
{
    "tag_ids": ["{{test_tag_id}}", "{{test_tag_id_2}}"]
}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Remove specific tag from repo - success
DELETE {{base_url}}/api/v1/repos/{{repo_id}}/tags/{{test_tag_id_2}}
Authorization: Bearer {{user_token}}
HTTP 204

# Remove tag - not found (already removed or not on repo)
DELETE {{base_url}}/api/v1/repos/{{repo_id}}/tags/00000000-0000-0000-0000-000000000000
Authorization: Bearer {{user_token}}
HTTP 404

# Repo tags - unauthorized (no token)
GET {{base_url}}/api/v1/repos/{{repo_id}}/tags
HTTP 401

# Clean up: Remove remaining tag
DELETE {{base_url}}/api/v1/repos/{{repo_id}}/tags/{{test_tag_id}}
Authorization: Bearer {{user_token}}
HTTP 204

# Clean up: Delete test tags
DELETE {{base_url}}/api/v1/tags/{{test_tag_id}}?force=true
Authorization: Bearer {{user_token}}
HTTP 204

DELETE {{base_url}}/api/v1/tags/{{test_tag_id_2}}?force=true
Authorization: Bearer {{user_token}}
HTTP 204
