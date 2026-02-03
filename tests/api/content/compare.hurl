# Content Compare API Tests

# Compare refs - success (main to main shows no differences)
GET {{base_url}}/api/v1/repos/{{repo_id}}/compare/main...main
Authorization: Bearer {{principal_token}}
HTTP 200
[Asserts]
jsonpath "$.data.base_sha" isString
jsonpath "$.data.head_sha" isString
jsonpath "$.data.ahead_by" isInteger
jsonpath "$.data.commits" isCollection

# Compare - invalid spec format
GET {{base_url}}/api/v1/repos/{{repo_id}}/compare/invalid-spec
Authorization: Bearer {{principal_token}}
HTTP 400

# Compare - not found (invalid ref)
GET {{base_url}}/api/v1/repos/{{repo_id}}/compare/nonexistent...main
Authorization: Bearer {{principal_token}}
HTTP 404

# Compare - not found (invalid repo)
GET {{base_url}}/api/v1/repos/00000000-0000-0000-0000-000000000000/compare/main...main
Authorization: Bearer {{principal_token}}
HTTP 404

# Compare - unauthorized (private repo, no token)
GET {{base_url}}/api/v1/repos/{{repo_id}}/compare/main...main
HTTP 401
