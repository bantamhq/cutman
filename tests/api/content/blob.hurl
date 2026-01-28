# Content Blob API Tests

# Get blob - success (JSON mode)
GET {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/README.md
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.sha" isString
jsonpath "$.data.size" isInteger
jsonpath "$.data.content" isString
jsonpath "$.data.is_binary" == false

# Get blob - success (raw mode)
GET {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/README.md?raw=true
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
header "Content-Type" contains "text/markdown"

# Get blob in subdirectory - success
GET {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/src/main.rs
Authorization: Bearer {{user_token}}
HTTP 200
[Asserts]
jsonpath "$.data.sha" isString
jsonpath "$.data.content" isString

# Get blob - not found (invalid ref)
GET {{base_url}}/api/v1/repos/{{repo_id}}/blob/nonexistent-branch/README.md
Authorization: Bearer {{user_token}}
HTTP 404

# Get blob - not found (invalid path)
GET {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/nonexistent.txt
Authorization: Bearer {{user_token}}
HTTP 404

# Blob - not found (invalid repo)
GET {{base_url}}/api/v1/repos/00000000-0000-0000-0000-000000000000/blob/main/README.md
Authorization: Bearer {{user_token}}
HTTP 404

# Blob - unauthorized (private repo, no token)
GET {{base_url}}/api/v1/repos/{{repo_id}}/blob/main/README.md
HTTP 401
