# Repo-Scoped Token Tests

# Setup: a second repo owned by the same principal
POST {{base_url}}/api/v1/repos
Authorization: Bearer {{principal_token}}
Content-Type: application/json
{
    "name": "scoped-other-{{test_suffix}}",
    "namespace": "{{principal_ns_name}}"
}
HTTP 201
[Captures]
other_repo_id: jsonpath "$.data.id"
other_repo_name: jsonpath "$.data.name"

# Create scoped token - allow without repo_id is rejected
POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "allow": ["repo:read"]
}
HTTP 400

# Create scoped token - repo_id without allow is rejected
POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "repo_id": "{{repo_id}}"
}
HTTP 400

# Create scoped token - unknown repo
POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "repo_id": "00000000-0000-0000-0000-000000000000",
    "allow": ["repo:read"]
}
HTTP 404

# Create scoped token - namespace permissions are not allowed
POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "repo_id": "{{repo_id}}",
    "allow": ["namespace:write"]
}
HTTP 400

# Create scoped token - invalid permission string
POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "repo_id": "{{repo_id}}",
    "allow": ["repo:everything"]
}
HTTP 400

# Create write-scoped token - success, scope is reported
POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "repo_id": "{{repo_id}}",
    "allow": ["repo:write"],
    "expires_in_seconds": 3600
}
HTTP 201
[Asserts]
jsonpath "$.data.token" startsWith "cutman_"
jsonpath "$.data.metadata.scope.repo_id" == "{{repo_id}}"
jsonpath "$.data.metadata.scope.allow" includes "repo:write"
jsonpath "$.data.metadata.expires_at" exists
[Captures]
write_token: jsonpath "$.data.token"
write_token_id: jsonpath "$.data.metadata.id"

# Create read-scoped token - success
POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "repo_id": "{{repo_id}}",
    "allow": ["repo:read"]
}
HTTP 201
[Captures]
read_token: jsonpath "$.data.token"

# Scope is reported when fetching the token
GET {{base_url}}/api/v1/admin/tokens/{{write_token_id}}
Authorization: Bearer {{admin_token}}
HTTP 200
[Asserts]
jsonpath "$.data.scope.repo_id" == "{{repo_id}}"

# Write-scoped token can fetch its repo
GET {{base_url}}/git/{{principal_ns_name}}/{{repo_name}}/info/refs?service=git-upload-pack
[BasicAuth]
x-token: {{write_token}}
HTTP 200

# Write-scoped token can push to its repo
GET {{base_url}}/git/{{principal_ns_name}}/{{repo_name}}/info/refs?service=git-receive-pack
[BasicAuth]
x-token: {{write_token}}
HTTP 200

# Write-scoped token cannot fetch a different repo
GET {{base_url}}/git/{{principal_ns_name}}/{{other_repo_name}}/info/refs?service=git-upload-pack
[BasicAuth]
x-token: {{write_token}}
HTTP 403

# Write-scoped token cannot push to a different repo
GET {{base_url}}/git/{{principal_ns_name}}/{{other_repo_name}}/info/refs?service=git-receive-pack
[BasicAuth]
x-token: {{write_token}}
HTTP 403

# Read-scoped token can fetch its repo
GET {{base_url}}/git/{{principal_ns_name}}/{{repo_name}}/info/refs?service=git-upload-pack
[BasicAuth]
x-token: {{read_token}}
HTTP 200

# Read-scoped token cannot push to its repo
GET {{base_url}}/git/{{principal_ns_name}}/{{repo_name}}/info/refs?service=git-receive-pack
[BasicAuth]
x-token: {{read_token}}
HTTP 403

# Scoped token can read content of its repo
GET {{base_url}}/api/v1/repos/{{repo_id}}/refs
Authorization: Bearer {{write_token}}
HTTP 200

# Scoped token cannot read content of a different repo
GET {{base_url}}/api/v1/repos/{{other_repo_id}}/refs
Authorization: Bearer {{write_token}}
HTTP 403

# Scoped token cannot list repos
GET {{base_url}}/api/v1/repos
Authorization: Bearer {{write_token}}
HTTP 403

# Scoped token cannot create repos
POST {{base_url}}/api/v1/repos
Authorization: Bearer {{write_token}}
Content-Type: application/json
{
    "name": "scoped-escape-{{test_suffix}}",
    "namespace": "{{principal_ns_name}}"
}
HTTP 403

# Scoped token cannot use the principal repo API for its own repo
GET {{base_url}}/api/v1/repos/{{repo_id}}
Authorization: Bearer {{write_token}}
HTTP 403

# Scoped token cannot reach admin routes
GET {{base_url}}/api/v1/admin/tokens
Authorization: Bearer {{write_token}}
HTTP 403

# Expired scoped token is rejected
POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "repo_id": "{{repo_id}}",
    "allow": ["repo:write"],
    "expires_in_seconds": 0
}
HTTP 201
[Captures]
expired_token: jsonpath "$.data.token"

GET {{base_url}}/git/{{principal_ns_name}}/{{repo_name}}/info/refs?service=git-upload-pack
[BasicAuth]
x-token: {{expired_token}}
HTTP 401

# Setup: a repo owned by another principal
POST {{base_url}}/api/v1/admin/principals
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "namespace_name": "scoped-outsider-{{test_suffix}}"
}
HTTP 201
[Captures]
outsider_id: jsonpath "$.data.id"

POST {{base_url}}/api/v1/admin/principals/{{outsider_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{}
HTTP 201
[Captures]
outsider_token: jsonpath "$.data.token"

POST {{base_url}}/api/v1/repos
Authorization: Bearer {{outsider_token}}
Content-Type: application/json
{
    "name": "outsider-repo",
    "namespace": "scoped-outsider-{{test_suffix}}"
}
HTTP 201
[Captures]
outsider_repo_id: jsonpath "$.data.id"

GET {{base_url}}/git/scoped-outsider-{{test_suffix}}/outsider-repo/info/refs?service=git-receive-pack
[BasicAuth]
x-token: {{outsider_token}}
HTTP 200

# Scope cannot exceed the principal's permissions on the repo
POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "repo_id": "{{outsider_repo_id}}",
    "allow": ["repo:read"]
}
HTTP 400

# Grant read on the outsider repo, then a read-scoped token is allowed
POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/repo-grants
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "repo_id": "{{outsider_repo_id}}",
    "allow": ["repo:read"]
}
HTTP 200

POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "repo_id": "{{outsider_repo_id}}",
    "allow": ["repo:write"]
}
HTTP 400

POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "repo_id": "{{outsider_repo_id}}",
    "allow": ["repo:read"]
}
HTTP 201
[Captures]
outsider_read_token: jsonpath "$.data.token"

GET {{base_url}}/git/scoped-outsider-{{test_suffix}}/outsider-repo/info/refs?service=git-upload-pack
[BasicAuth]
x-token: {{outsider_read_token}}
HTTP 200

# Revoking the principal's grant also revokes the scoped token's access
DELETE {{base_url}}/api/v1/admin/principals/{{principal_id}}/repo-grants/{{outsider_repo_id}}
Authorization: Bearer {{admin_token}}
HTTP 204

GET {{base_url}}/git/scoped-outsider-{{test_suffix}}/outsider-repo/info/refs?service=git-upload-pack
[BasicAuth]
x-token: {{outsider_read_token}}
HTTP 403

# Deleting a repo deletes the tokens scoped to it
POST {{base_url}}/api/v1/admin/principals/{{principal_id}}/tokens
Authorization: Bearer {{admin_token}}
Content-Type: application/json
{
    "repo_id": "{{other_repo_id}}",
    "allow": ["repo:read"]
}
HTTP 201
[Captures]
other_token_id: jsonpath "$.data.metadata.id"

DELETE {{base_url}}/api/v1/repos/{{other_repo_id}}
Authorization: Bearer {{principal_token}}
HTTP 204

GET {{base_url}}/api/v1/admin/tokens/{{other_token_id}}
Authorization: Bearer {{admin_token}}
HTTP 404
