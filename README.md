# Cutman

A Git hosting server.

## Admin CLI Commands

```
cutman admin
├── init                    Initialize server (create database and admin token)
├── user
│   ├── add                 Add a new user with namespace and optional token
│   └── remove              Remove a user
├── token
│   ├── create              Create a new access token
│   └── revoke              Revoke an access token
├── namespace
│   ├── add                 Add a new shared namespace
│   └── remove              Remove a shared namespace
├── permission
│   ├── grant               Grant permissions to a user on a namespace
│   └── revoke              Revoke a user's permissions on a namespace
└── info                    Show server status information
```

### Common Flags

| Flag | Description |
|------|-------------|
| `--data-dir` | Data directory for database and repositories (default: `./data`) |
| `--list` | List existing items instead of performing action |
| `--json` | Output as JSON (for scripting) |
| `--non-interactive` | Skip interactive prompts (requires all args via flags) |
| `-y, --yes` | Skip confirmation prompts for destructive actions |

### Examples

```bash
# Initialize the server
cutman admin init

# Add a user with a token
cutman admin user add --username alice --create-token

# List all users as JSON
cutman admin user add --list --json

# Create a shared namespace
cutman admin namespace add --name shared-libs

# Grant permissions (non-interactive)
cutman admin permission grant \
  --user-id <id> \
  --namespace-id <id> \
  --permissions "repo:read,repo:write"

# Show server status
cutman admin info
```
