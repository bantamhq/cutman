# Content Commits API Tests

# List commits - success
GET {{base_url}}/api/v1/repos/{{repo_id}}/commits
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection
jsonpath "$.has_more" isBoolean

# List commits with ref filter
GET {{base_url}}/api/v1/repos/{{repo_id}}/commits?ref=main
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# List commits with path filter
GET {{base_url}}/api/v1/repos/{{repo_id}}/commits?path=README.md
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Get specific commit - success
GET {{base_url}}/api/v1/repos/{{repo_id}}/commits/{{commit_sha}}
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.sha" isString
jsonpath "$.data.message" isString
jsonpath "$.data.author" isCollection
jsonpath "$.data.committer" isCollection

# Get commit - not found (invalid sha)
GET {{base_url}}/api/v1/repos/{{repo_id}}/commits/0000000000000000000000000000000000000000
Authorization: Bearer {{user_token}}
HTTP 404

# Get commit diff - success
GET {{base_url}}/api/v1/repos/{{repo_id}}/commits/{{commit_sha}}/diff
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.head_sha" isString
jsonpath "$.data.patch" isString
jsonpath "$.data.stats" isCollection

# Commits - not found (invalid repo)
GET {{base_url}}/api/v1/repos/00000000-0000-0000-0000-000000000000/commits
Authorization: Bearer {{user_token}}
HTTP 404

# Commits - unauthorized (private repo, no token)
GET {{base_url}}/api/v1/repos/{{repo_id}}/commits
HTTP 401
