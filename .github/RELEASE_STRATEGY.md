# Release Strategy for cascette-rs

This document outlines the unified release strategy for the cascette-rs project.

## Tag-Based Release Types

The release system uses semantic tag prefixes to automatically determine what to release:

### 1. Full Workspace Release: `v{version}`

**Example**: `v0.3.0`

- **Triggers**: Both library and CLI releases
- **Libraries**: Published to crates.io
- **CLI**: Cross-platform binaries with ephemeral signing
- **Use Case**: Major version releases, coordinated updates

### 2. CLI-Only Release: `ngdp-client-v{version}`

**Example**: `ngdp-client-v0.3.0`

- **Triggers**: CLI binary release only
- **Libraries**: Not published to crates.io
- **CLI**: Cross-platform binaries with ephemeral signing
- **Use Case**: CLI improvements, bug fixes, new features

### 3. Libraries-Only Release: `libs-v{version}`

**Example**: `libs-v0.3.0`

- **Triggers**: Library release only
- **Libraries**: Published to crates.io
- **CLI**: No binary release
- **Use Case**: Library API changes, dependency updates

## Manual Release Control

### Workflow Dispatch Options

Use GitHub Actions → "Release" → "Run workflow" for manual control:

- **Version**: Semantic version (e.g., 0.3.0)
- **Release Type**:
  - `all`: Both libraries and CLI
  - `libraries`: Libraries only
  - `cli`: CLI only
- **Dry Run**: Test without publishing (default: false)
- **Skip Tests**: Skip test suite (default: false, use with caution)

### Examples

```bash
# CLI-only release (recommended for most updates)
git tag ngdp-client-v0.3.0 -m "CLI improvements and ephemeral signing"
git push --tags

# Full workspace release
git tag v0.3.0 -m "Major release with new features"
git push --tags

# Libraries-only release
git tag libs-v0.3.0 -m "Library API updates"
git push --tags

# Dry run test (manual workflow only)
# Go to GitHub Actions → Release → Run workflow
# Set: version=0.3.0, release-type=cli, dry-run=true
```

## Release Process

### Automated (Tag-Based)

1. **Create and push tag** using appropriate prefix
2. **Release workflow triggers** automatically
3. **System determines** what to release based on tag prefix
4. **Builds, tests, and publishes** according to release type

### Manual (Workflow Dispatch)

1. **Go to GitHub Actions** → "Release" workflow
2. **Click "Run workflow"**
3. **Configure options** (version, type, dry-run)
4. **Monitor execution** and verify results

## Security Features

### Ephemeral Signing (CLI Releases)

- **New keypair** generated for each release
- **AGE encryption** protects private keys
- **Complete audit trail** in signature metadata
- **No long-term key management** required

### Concurrency Protection

- **Prevents overlapping releases** to avoid conflicts
- **Ensures consistent state** during release process

## Quality Gates

### Required Checks

- ✅ **Version consistency** across all workspace crates
- ✅ **Full test suite** (unless explicitly skipped)
- ✅ **Cross-platform builds** for CLI releases
- ✅ **Signature verification** for all binaries

### Optional Features

- 🔄 **Dry run mode** for testing release process
- 🔄 **Skip tests** for emergency releases (use carefully)
- 🔄 **Partial releases** (libraries or CLI only)

## Troubleshooting

### Failed Library Publishing

- **Check crates.io authentication** (`CRATES_IO_TOKEN`)
- **Verify version numbers** haven't been published before
- **Use dry run** to test publishing logic first

### Failed CLI Builds

- **Check AGE keys** are properly configured
- **Verify ephemeral signing** scripts are executable
- **Monitor cross-compilation** for platform-specific issues

### Version Conflicts

- **Ensure all workspace crates** have consistent versions
- **Check for existing releases** with same version
- **Use semantic versioning** for proper ordering

## Best Practices

### For Regular Development

- **Use CLI-only releases** (`ngdp-client-v*`) for most updates
- **Test with dry runs** before production releases
- **Keep release notes updated** in commits

### For Major Releases

- **Coordinate library and CLI** versions with full release (`v*`)
- **Ensure comprehensive testing** across all platforms
- **Communicate breaking changes** clearly

### For Emergency Fixes

- **Use CLI-only releases** for quick fixes
- **Consider skip-tests** only if absolutely necessary
- **Follow up with proper testing** in next release

## Migration from Old System

### Deprecated Workflows

- ❌ **`release-libraries.yml`** - Removed (functionality moved to main release.yml)
- ❌ **Manual version coordination** - Now automated based on workspace

### New Features

- ✅ **Unified release workflow** with multiple trigger types
- ✅ **Automatic release type detection** from tags
- ✅ **Dry run capabilities** for testing
- ✅ **Enhanced concurrency control** and error handling
