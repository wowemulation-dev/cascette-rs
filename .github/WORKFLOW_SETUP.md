# GitHub Actions Workflow Setup

## Release-plz Workflow

The Release-plz workflow requires a Personal Access Token (PAT) to create pull requests.

### Setup Instructions

1. Create a Personal Access Token:
   - Go to GitHub Settings > Developer settings > Personal access tokens > Tokens (classic)
   - Click "Generate new token (classic)"
   - Give it a descriptive name like "Release-plz Token"
   - Select the following scopes:
     - `repo` (Full control of private repositories)
     - `workflow` (Update GitHub Action workflows)
   - Generate the token and copy it

2. Add the token to repository secrets:
   - Go to repository Settings > Secrets and variables > Actions
   - Click "New repository secret"
   - Name: `RELEASE_PLZ_TOKEN`
   - Value: Paste the token you copied
   - Click "Add secret"

### Why is this needed?

GitHub Actions using the default `GITHUB_TOKEN` cannot create or approve pull requests to prevent automated workflows from bypassing branch protection rules. Release-plz needs to create pull requests for version updates, so it requires a PAT with appropriate permissions.

## Cross-Platform Build Workflow

The cross-platform build workflow has been updated to use `rustls` instead of OpenSSL for TLS support. This allows the project to be built for various platforms without requiring OpenSSL to be cross-compiled.

### Changes Made

- All `reqwest` dependencies now use `rustls-tls` feature instead of the default `native-tls`
- This removes the OpenSSL dependency completely
- Enables successful cross-compilation for targets like:
  - `aarch64-unknown-linux-musl`
  - `armv7-unknown-linux-gnueabihf`
  - And other cross-compilation targets
