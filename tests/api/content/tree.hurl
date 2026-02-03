# Content Tree API Tests

# Get root tree - success
GET {{base_url}}/api/v1/repos/{{repo_id}}/tree/main
Authorization: Bearer {{principal_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Get tree with depth parameter
GET {{base_url}}/api/v1/repos/{{repo_id}}/tree/main?depth=2
Authorization: Bearer {{principal_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Get subdirectory tree - success
GET {{base_url}}/api/v1/repos/{{repo_id}}/tree/main/src
Authorization: Bearer {{principal_token}}
HTTP 200
[Asserts]
jsonpath "$.data" isCollection

# Get tree - not found (invalid ref)
GET {{base_url}}/api/v1/repos/{{repo_id}}/tree/nonexistent-branch
Authorization: Bearer {{principal_token}}
HTTP 404

# Get tree - not found (invalid path)
GET {{base_url}}/api/v1/repos/{{repo_id}}/tree/main/nonexistent/path
Authorization: Bearer {{principal_token}}
HTTP 404

# Tree - not found (invalid repo)
GET {{base_url}}/api/v1/repos/00000000-0000-0000-0000-000000000000/tree/main
Authorization: Bearer {{principal_token}}
HTTP 404

# Tree - unauthorized (private repo, no token)
GET {{base_url}}/api/v1/repos/{{repo_id}}/tree/main
HTTP 401
