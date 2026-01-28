# LFS Objects API Tests

# Upload object with valid content (sha256 of "123" = a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3)
PUT {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/objects/a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3
Authorization: {{git_auth_header}}
Content-Length: 3
```
123
```
HTTP 200

# Verify uploaded object
POST {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/verify
Authorization: {{git_auth_header}}
Content-Type: application/vnd.git-lfs+json
{
    "oid": "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
    "size": 3
}
HTTP 200

# Verify with size mismatch
POST {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/verify
Authorization: {{git_auth_header}}
Content-Type: application/vnd.git-lfs+json
{
    "oid": "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
    "size": 100
}
HTTP 400
[Asserts]
jsonpath "$.message" contains "Size mismatch"

# Download existing object
GET {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/objects/a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3
Authorization: {{git_auth_header}}
HTTP 200
[Asserts]
header "Content-Type" == "application/octet-stream"
header "Content-Length" == "3"
body == "123"

# Batch download - object now exists
POST {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/objects/batch
Authorization: {{git_auth_header}}
Content-Type: application/vnd.git-lfs+json
{
    "operation": "download",
    "objects": [
        {"oid": "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3", "size": 3}
    ]
}
HTTP 200
[Asserts]
jsonpath "$.objects[0].actions.download.href" isString

# Batch upload - object already exists (no upload action)
POST {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/objects/batch
Authorization: {{git_auth_header}}
Content-Type: application/vnd.git-lfs+json
{
    "operation": "upload",
    "objects": [
        {"oid": "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3", "size": 3}
    ]
}
HTTP 200
[Asserts]
jsonpath "$.objects[0].actions" not exists

# Download non-existent object
GET {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/objects/0000000000000000000000000000000000000000000000000000000000000000
Authorization: {{git_auth_header}}
HTTP 404
[Asserts]
jsonpath "$.message" == "Object not found"

# Upload with invalid OID
PUT {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/objects/invalid
Authorization: {{git_auth_header}}
Content-Length: 3
```
123
```
HTTP 400
[Asserts]
jsonpath "$.message" == "Invalid OID format"

# Upload with hash mismatch
PUT {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/objects/0000000000000000000000000000000000000000000000000000000000000000
Authorization: {{git_auth_header}}
Content-Length: 3
```
123
```
HTTP 400
[Asserts]
jsonpath "$.message" == "Hash mismatch"

# Verify non-existent object
POST {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/verify
Authorization: {{git_auth_header}}
Content-Type: application/vnd.git-lfs+json
{
    "oid": "0000000000000000000000000000000000000000000000000000000000000000",
    "size": 3
}
HTTP 404
[Asserts]
jsonpath "$.message" == "Object not found"

# Download unauthorized - no token on private repo
GET {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/objects/a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3
HTTP 401
[Asserts]
header "WWW-Authenticate" contains "Basic"
