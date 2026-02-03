# Content Archive API Tests

# Get archive - zip format (default)
GET {{base_url}}/api/v1/repos/{{repo_id}}/archive/main
Authorization: Bearer {{principal_token}}
HTTP 200
[Asserts]
header "Content-Type" contains "application/zip"
header "Content-Disposition" contains "attachment"

# Get archive - explicit zip format
GET {{base_url}}/api/v1/repos/{{repo_id}}/archive/main?format=zip
Authorization: Bearer {{principal_token}}
HTTP 200
[Asserts]
header "Content-Type" contains "application/zip"

# Get archive - tar.gz format
GET {{base_url}}/api/v1/repos/{{repo_id}}/archive/main?format=tar.gz
Authorization: Bearer {{principal_token}}
HTTP 200
[Asserts]
header "Content-Type" contains "application/gzip"
header "Content-Disposition" contains "attachment"

# Archive - not found (invalid ref)
GET {{base_url}}/api/v1/repos/{{repo_id}}/archive/nonexistent-branch
Authorization: Bearer {{principal_token}}
HTTP 404

# Archive - not found (invalid repo)
GET {{base_url}}/api/v1/repos/00000000-0000-0000-0000-000000000000/archive/main
Authorization: Bearer {{principal_token}}
HTTP 404

# Archive - unauthorized (private repo, no token)
GET {{base_url}}/api/v1/repos/{{repo_id}}/archive/main
HTTP 401
