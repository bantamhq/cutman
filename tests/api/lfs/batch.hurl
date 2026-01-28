# LFS Batch API Tests

# Batch download request - object doesn't exist
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
header "Content-Type" contains "application/vnd.git-lfs+json"
jsonpath "$.transfer" == "basic"
jsonpath "$.objects[0].oid" == "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
jsonpath "$.objects[0].error.code" == 404
jsonpath "$.objects[0].error.message" == "Object not found"

# Batch upload request - get upload action
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
jsonpath "$.transfer" == "basic"
jsonpath "$.objects[0].oid" == "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
jsonpath "$.objects[0].actions.upload.href" isString
jsonpath "$.objects[0].actions.verify.href" isString

# Batch with invalid operation
POST {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/objects/batch
Authorization: {{git_auth_header}}
Content-Type: application/vnd.git-lfs+json
{
    "operation": "invalid",
    "objects": [
        {"oid": "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3", "size": 3}
    ]
}
HTTP 400
[Asserts]
jsonpath "$.message" == "Invalid operation"

# Batch with invalid OID format
POST {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/objects/batch
Authorization: {{git_auth_header}}
Content-Type: application/vnd.git-lfs+json
{
    "operation": "upload",
    "objects": [
        {"oid": "invalid", "size": 3}
    ]
}
HTTP 200
[Asserts]
jsonpath "$.objects[0].error.code" == 422
jsonpath "$.objects[0].error.message" == "Invalid OID format"

# Batch unauthorized - no token
POST {{base_url}}/git/{{user_ns_name}}/{{repo_name}}.git/info/lfs/objects/batch
Content-Type: application/vnd.git-lfs+json
{
    "operation": "upload",
    "objects": [
        {"oid": "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3", "size": 3}
    ]
}
HTTP 401
[Asserts]
header "WWW-Authenticate" contains "Basic"
jsonpath "$.message" == "Authentication required"

# Batch on non-existent repo
POST {{base_url}}/git/{{user_ns_name}}/nonexistent.git/info/lfs/objects/batch
Authorization: {{git_auth_header}}
Content-Type: application/vnd.git-lfs+json
{
    "operation": "download",
    "objects": []
}
HTTP 404
[Asserts]
jsonpath "$.message" == "Repository not found"
