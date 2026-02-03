# Content Refs API Tests

# List refs - success
GET {{base_url}}/api/v1/repos/{{repo_id}}/refs
Authorization: Bearer {{principal_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# List refs - not found (invalid repo)
GET {{base_url}}/api/v1/repos/00000000-0000-0000-0000-000000000000/refs
Authorization: Bearer {{principal_token}}
HTTP 404

# Refs - unauthorized (private repo, no token)
GET {{base_url}}/api/v1/repos/{{repo_id}}/refs
HTTP 401
