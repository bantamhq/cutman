# Git Smart HTTP Protocol Tests

# Info refs for upload-pack (clone/fetch)
GET {{base_url}}/git/{{principal_ns_name}}/{{repo_name}}/info/refs?service=git-upload-pack
Authorization: {{git_auth_header}}
HTTP 200
[Asserts]
header "Content-Type" contains "application/x-git-upload-pack-advertisement"

# Info refs for receive-pack (push)
GET {{base_url}}/git/{{principal_ns_name}}/{{repo_name}}/info/refs?service=git-receive-pack
Authorization: {{git_auth_header}}
HTTP 200
[Asserts]
header "Content-Type" contains "application/x-git-receive-pack-advertisement"

# Info refs - unauthorized (no auth)
GET {{base_url}}/git/{{principal_ns_name}}/{{repo_name}}/info/refs?service=git-upload-pack
HTTP 401

# Info refs - not found (invalid namespace)
GET {{base_url}}/git/nonexistent/{{repo_name}}/info/refs?service=git-upload-pack
Authorization: {{git_auth_header}}
HTTP 404

# Info refs - not found (invalid repo)
GET {{base_url}}/git/{{principal_ns_name}}/nonexistent/info/refs?service=git-upload-pack
Authorization: {{git_auth_header}}
HTTP 404

# Upload-pack endpoint exists (binary protocol - just verify it accepts POST)
# Note: Full git-upload-pack requires proper binary pack negotiation
POST {{base_url}}/git/{{principal_ns_name}}/{{repo_name}}/git-upload-pack
Authorization: {{git_auth_header}}
Content-Type: application/x-git-upload-pack-request
```
0000
```
HTTP 200
[Asserts]
header "Content-Type" contains "application/x-git-upload-pack-result"

# Receive-pack endpoint - unauthorized
POST {{base_url}}/git/{{principal_ns_name}}/{{repo_name}}/git-receive-pack
Content-Type: application/x-git-receive-pack-request
```
0000
```
HTTP 401
