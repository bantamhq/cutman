# Cutman

> A lightweight, self-hostable git server built for organizing code, experiments, and AI context.

In boxing, the [cutman](https://en.wikipedia.org/wiki/Cutman) is the person in your corner who keeps you in the fight — keeping everything organized so you can focus on what matters. That's what this project does for your code.

## Why Cutman?

GitHub and GitLab are built for collaboration, CI/CD, and project discovery. But if you're a developer with lots of small projects, experiments, and AI workflow artifacts, you've probably felt the friction:

- **No organization**: Repos pile up in a flat list with no way to group them
- **Too much ceremony**: Creating a repo feels heavy when you just want to throw something somewhere
- **Self-hosting is complex**: Gitea and GitLab are full-featured but require real infrastructure
- **AI workflows generate artifacts**: Skills, prompts, CLAUDE.md files, experiments — they need a home too

Cutman is a git server without the weight of a web interface. Nestable folders. Tags. CLI-first. Single binary. SQLite. Done.

## Features

- **Nestable folders** — Organize repos hierarchically (`experiments/react`, `skills/claude-code`)
- **Tags** — Categorize across folder boundaries
- **Single binary** — Server and CLI in one, no external dependencies
- **SQLite storage** — No database server needed
- **Full REST API** — Build tools on top, automate everything
- **Multi-user & namespaces** — Personal namespaces plus shared orgs with fine-grained permissions
- **Git LFS support** — Large files handled
- **CLI-first** — No web UI to maintain or navigate

## Quick Start

```bash
# Install
cargo install cutman

# Initialize server
cutman admin init

# Start server
cutman serve

# Login from your machine
cutman login

# Create a new repo. Automatically creates a git repo and pushes if one doesn't already exist.
cutman new experiments/my-idea

```

## Built for AI Workflows

AI-assisted development generates artifacts at a different pace than traditional coding:

- System prompts and CLAUDE.md files that evolve per-project
- Custom skills and slash commands you want to reuse across projects
- Experimental agent configurations worth versioning
- Quick prototypes and throwaway experiments

Cutman gives these artifacts a proper home:

- **Skill library** — Keep a `skills/` folder with all your Claude skills, easily copy into `.claude/` as needed
- **Prompt versioning** — Track how your system prompts evolve without cluttering your main projects
- **Low-friction experiments** — Spin up repos without the ceremony of GitHub
- **Ringside integration** (coming) — Export folders/tags as [Ringside](https://github.com/bantamhq/ringside)-compatible bundles

## Deployment

Cutman is designed to be trivially self-hostable.

**On [Sprites.dev](https://sprites.dev)**:
```bash
# Coming Soon
```

**On any VPS**: Single binary, SQLite database, minimal resources. No Docker required.

**With Docker** (Dokploy, Coolify, etc.):
```bash
docker build -t cutman .
docker run -d -p 8080:8080 -v cutman-data:/data \
  -e CUTMAN_PUBLIC_BASE_URL=https://git.example.com cutman
```

The container initializes the server on first start and writes the admin token to `/data/.admin_token` (it's also printed to the logs once). Keep `/data` on a persistent volume: it holds the database and every repository. `CUTMAN_PORT` changes the port, and any `cutman` command can be run with `docker exec <container> cutman ...`.

## CLI Reference

| Command | Description |
|---------|-------------|
| `cutman serve` | Run the server |
| `cutman login` | Authenticate with a server |
| `cutman new <namespace/repo>` | Create a new repository |
| `cutman repo clone` | Clone a repository |
| `cutman repo delete` | Delete a repository |
| `cutman repo move --folder` | Move repo to a folder |
| `cutman repo tag --tags` | Tag a repository |
| `cutman folder create` | Create a folder |
| `cutman folder list` | List folders |
| `cutman folder delete` | Delete a folder |
| `cutman tag create` | Create a tag |
| `cutman tag delete` | Delete a tag |

Admin commands (direct database access):

| Command | Description |
|---------|-------------|
| `cutman admin init` | Initialize the server |
| `cutman admin principal add` | Create a principal (user account) |
| `cutman admin token create` | Generate a token |
| `cutman admin namespace add` | Create a shared namespace |
| `cutman admin permission grant` | Grant namespace/repo access |

## API

Cutman has a comprehensive REST API covering everything: repos, folders, tags, principals, namespaces, permissions, and git content browsing (commits, trees, blobs, blame, diffs).

### Repo-scoped tokens

For handing short-lived git access to an agent, CI job or sandbox, an admin can create a token limited to a single repository:

```bash
curl -X POST https://git.example.com/api/v1/admin/principals/$PRINCIPAL_ID/tokens \
  -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"repo_id": "'$REPO_ID'", "allow": ["repo:write"], "expires_in_seconds": 3600}'
```

A scoped token only works for git operations and content reads on that repository, can never do more than its principal is allowed to, and is deleted along with the repository. Use it as the password in a git remote: `https://x-token:$TOKEN@git.example.com/git/<namespace>/<repo>`.

See openapi.yaml for full documentation.

## Related Projects

- [Ringside](https://github.com/bantamhq/ringside) — Pairs well with Cutman for managing AI context bundles

---

MIT License
