# Content Blame API Tests

# Get blame - success
GET {{base_url}}/api/v1/repos/{{repo_id}}/blame/main/README.md
Authorization: Bearer {{principal_token}}
HTTP 200
[Asserts]
jsonpath "$.data.lines" isCollection

# Get blame - not found (invalid ref)
GET {{base_url}}/api/v1/repos/{{repo_id}}/blame/nonexistent-branch/README.md
Authorization: Bearer {{principal_token}}
HTTP 404

# Get blame - not found (invalid path)
GET {{base_url}}/api/v1/repos/{{repo_id}}/blame/main/nonexistent.txt
Authorization: Bearer {{principal_token}}
HTTP 404

# Blame - not found (invalid repo)
GET {{base_url}}/api/v1/repos/00000000-0000-0000-0000-000000000000/blame/main/README.md
Authorization: Bearer {{principal_token}}
HTTP 404

# Blame - unauthorized (private repo, no token)
GET {{base_url}}/api/v1/repos/{{repo_id}}/blame/main/README.md
HTTP 401
