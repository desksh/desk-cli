# Branch Protection Rules

This document outlines the branch protection rules for the desk-cli repository.

## Branch Strategy

We follow a GitFlow-inspired branching model:

```
production ← staging ← develop ← feature/*
     ↑          ↑         ↑
  hotfix/*   release/*  bugfix/*
```

### Branch Descriptions

| Branch | Purpose | Protected |
|--------|---------|-----------|
| `main` | Production-ready code, mirrors `production` | ✅ |
| `production` | Production deployments | ✅ |
| `staging` | Pre-production testing | ✅ |
| `develop` | Active development integration | ✅ |
| `feature/*` | New features | ❌ |
| `bugfix/*` | Bug fixes for develop | ❌ |
| `hotfix/*` | Emergency production fixes | ❌ |
| `release/*` | Release preparation | ❌ |

## Protection Rules

### `main` / `production` Branch

```yaml
Required Settings:
  - Require pull request before merging: true
  - Required approvals: 2
  - Dismiss stale reviews on new commits: true
  - Require review from CODEOWNERS: true
  - Require status checks to pass: true
    Required checks:
      - CI / Check
      - CI / Format
      - CI / Clippy
      - CI / Test (ubuntu-latest)
      - CI / Test (macos-latest)
      - CI / Test (windows-latest)
      - CI / Security Audit
  - Require branches to be up to date: true
  - Require signed commits: recommended
  - Require linear history: true
  - Do not allow bypassing: true
  - Restrict who can push: maintainers only
  - Allow force pushes: false
  - Allow deletions: false
```

### `staging` Branch

```yaml
Required Settings:
  - Require pull request before merging: true
  - Required approvals: 1
  - Dismiss stale reviews on new commits: true
  - Require status checks to pass: true
    Required checks:
      - CI / Check
      - CI / Format
      - CI / Clippy
      - CI / Test (ubuntu-latest)
  - Require branches to be up to date: true
  - Allow force pushes: false
  - Allow deletions: false
```

### `develop` Branch

```yaml
Required Settings:
  - Require pull request before merging: true
  - Required approvals: 1
  - Require status checks to pass: true
    Required checks:
      - CI / Check
      - CI / Format
      - CI / Clippy
      - CI / Test (ubuntu-latest)
  - Require branches to be up to date: false
  - Allow force pushes: false
  - Allow deletions: false
```

## Setting Up Branch Protection

### Via GitHub UI

1. Go to **Settings** → **Branches**
2. Click **Add branch protection rule**
3. Enter the branch name pattern
4. Configure the settings as specified above
5. Click **Create** or **Save changes**

### Via GitHub CLI

```bash
# Install GitHub CLI if not already installed
# https://cli.github.com/

# Set protection for main branch
gh api repos/{owner}/{repo}/branches/main/protection \
  --method PUT \
  --field required_status_checks='{"strict":true,"contexts":["CI / Check","CI / Format","CI / Clippy","CI / Test (ubuntu-latest)","CI / Test (macos-latest)","CI / Test (windows-latest)"]}' \
  --field enforce_admins=true \
  --field required_pull_request_reviews='{"required_approving_review_count":2,"dismiss_stale_reviews":true,"require_code_owner_reviews":true}' \
  --field restrictions=null \
  --field required_linear_history=true \
  --field allow_force_pushes=false \
  --field allow_deletions=false
```

## Workflow

### Feature Development

```bash
# Start from develop
git checkout develop
git pull origin develop

# Create feature branch
git checkout -b feature/my-feature

# Work on feature...
git commit -m "feat: add new feature"

# Push and create PR to develop
git push -u origin feature/my-feature
gh pr create --base develop --title "feat: add new feature"
```

### Release Process

```bash
# Create release branch from develop
git checkout develop
git pull origin develop
git checkout -b release/v1.0.0

# Update version, changelog, etc.
# Create PR to staging for testing
gh pr create --base staging --title "Release v1.0.0"

# After staging approval, create PR to production/main
gh pr create --base main --title "Release v1.0.0"
```

### Hotfix Process

```bash
# Create hotfix from production
git checkout production
git pull origin production
git checkout -b hotfix/critical-fix

# Fix the issue
git commit -m "fix: critical security issue"

# Create PR directly to production
gh pr create --base production --title "fix: critical security issue"

# After merge, also merge to develop
git checkout develop
git merge hotfix/critical-fix
git push origin develop
```

## Commit Message Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer(s)]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Code style (formatting, etc.)
- `refactor`: Code refactoring
- `perf`: Performance improvement
- `test`: Adding or updating tests
- `build`: Build system or dependencies
- `ci`: CI/CD configuration
- `chore`: Other changes

## Enforcement

These rules are enforced through:

1. **GitHub Branch Protection** - Prevents direct pushes
2. **Required Status Checks** - CI must pass
3. **Required Reviews** - Human approval required
4. **CODEOWNERS** - Specific owners for critical files
