# Content README API Tests

# Get README - success (auto-detect)
GET {{base_url}}/api/v1/repos/{{repo_id}}/readme
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.filename" isString
jsonpath "$.data.content" isString
jsonpath "$.data.sha" isString
jsonpath "$.data.size" isInteger

# README - not found (invalid repo)
GET {{base_url}}/api/v1/repos/00000000-0000-0000-0000-000000000000/readme
Authorization: Bearer {{user_token}}
HTTP 404

# README - unauthorized (private repo, no token)
GET {{base_url}}/api/v1/repos/{{repo_id}}/readme
HTTP 401
