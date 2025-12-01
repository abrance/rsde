# Release Guide

This document explains how to create a new release for the rsync project.

## Quick Start

Creating a release is simple - just push a version tag:

```bash
# 1. 确保所有更改已提交
git add .
git commit -m "Prepare for release v1.0.0"

# 2. 创建并推送 tag
git tag v1.0.0
git push origin v1.0.0
```

That's it! GitHub Actions will automatically:
- Build binaries for multiple platforms
- Create release tarballs
- Generate SHA256 checksums
- Create a GitHub Release with all artifacts
- Build and push Docker images
- Package and publish Helm chart

## Release Process Details

### 1. Tag Format

Tags must follow semantic versioning with a `v` prefix:
- `v1.0.0` - Major release
- `v1.1.0` - Minor release  
- `v1.1.1` - Patch release
- `v2.0.0-alpha.1` - Pre-release (marked as prerelease)
- `v2.0.0-beta.1` - Beta release (marked as prerelease)
- `v2.0.0-rc.1` - Release candidate (marked as prerelease)

### 2. Supported Platforms

The release workflow builds binaries for:
- **Linux x86_64** - `rsync-linux-x86_64.tar.gz`
- **Linux ARM64** - `rsync-linux-aarch64.tar.gz`
- **macOS x86_64** - `rsync-macos-x86_64.tar.gz`
- **macOS ARM64** (Apple Silicon) - `rsync-macos-aarch64.tar.gz`

### 3. Release Contents

Each tarball contains a `rsde/` directory with:
- `rsync` - The compiled binary (stripped)
- `README.md` - Documentation
- `example.toml` - Example configuration
- `VERSION` - Build information (version, commit, date, target)

Example structure:
```
rsde/
├── rsync           # Binary executable
├── README.md       # Documentation
├── example.toml    # Sample config
└── VERSION         # Build metadata
```

### 4. What Gets Created

When you push a tag, the workflow creates:

1. **GitHub Release** with:
   - All platform tarballs (`.tar.gz`)
   - SHA256 checksums (`.tar.gz.sha256`)
   - Helm chart package
   - Auto-generated changelog
   - Installation instructions

2. **Docker Images** pushed to GHCR:
   ```
   ghcr.io/<owner>/rsde/rsync:v1.0.0
   ghcr.io/<owner>/rsde/rsync:1.0.0
   ghcr.io/<owner>/rsde/rsync:1.0
   ghcr.io/<owner>/rsde/rsync:1
   ghcr.io/<owner>/rsde/rsync:sha-abc1234
   ```

3. **Helm Chart** pushed to GHCR:
   ```
   oci://ghcr.io/<owner>/rsde/helm/rsync:1.0.0
   ```

## Step-by-Step Release Process

### Preparing for Release

1. **Update version references** (if needed):
   ```bash
   cd rsync
   # Update Cargo.toml versions if changed
   vim Cargo.toml
   vim lib/rule/Cargo.toml
   vim lib/core/Cargo.toml
   ```

2. **Run all tests locally**:
   ```bash
   cd rsync
   make check  # runs fmt-check, clippy, and tests
   ```

3. **Update documentation** (if needed):
   - Update README.md
   - Update CHANGELOG.md (optional, auto-generated from commits)

4. **Commit changes**:
   ```bash
   git add .
   git commit -m "chore: prepare for release v1.0.0"
   git push origin master
   ```

### Creating the Release

1. **Create and push the tag**:
   ```bash
   # Replace 1.0.0 with your version
   git tag -a v1.0.0 -m "Release version 1.0.0"
   git push origin v1.0.0
   ```

2. **Monitor the workflow**:
   - Go to GitHub Actions tab
   - Watch the "Release" workflow
   - It takes ~10-15 minutes to complete all builds

3. **Verify the release**:
   - Go to GitHub Releases page
   - Check all artifacts are present
   - Download and test a binary

### After Release

1. **Announce the release**:
   - Update project README badges (if any)
   - Post to relevant channels
   - Update documentation sites

2. **Test the release artifacts**:
   ```bash
   # Download and verify
   wget https://github.com/<owner>/<repo>/releases/download/v1.0.0/rsync-linux-x86_64.tar.gz
   sha256sum -c rsync-linux-x86_64.tar.gz.sha256
   
   # Extract and test
   tar xzf rsync-linux-x86_64.tar.gz
   cd rsde
   ./rsync --version
   ```

3. **Test Docker image**:
   ```bash
   docker pull ghcr.io/<owner>/rsde/rsync:v1.0.0
   docker run --rm ghcr.io/<owner>/rsde/rsync:v1.0.0
   ```

4. **Test Helm chart**:
   ```bash
   helm install rsync oci://ghcr.io/<owner>/rsde/helm/rsync --version 1.0.0
   kubectl get pods
   ```

## Troubleshooting

### Release Failed

**Problem**: Build failed for one platform
**Solution**: 
1. Check the Actions logs for errors
2. Fix the issue in code
3. Delete the tag: `git tag -d v1.0.0 && git push origin :refs/tags/v1.0.0`
4. Fix and re-tag

**Problem**: Release created but artifacts missing
**Solution**:
1. Check if build jobs completed
2. Re-run failed jobs from GitHub Actions UI
3. Or delete release and re-push tag

### Tag Already Exists

**Problem**: Need to recreate a release
**Solution**:
```bash
# Delete local tag
git tag -d v1.0.0

# Delete remote tag
git push origin :refs/tags/v1.0.0

# Delete the release from GitHub UI

# Create new tag
git tag v1.0.0
git push origin v1.0.0
```

### Binary Won't Run

**Problem**: Downloaded binary doesn't execute
**Solution**:
```bash
# Make it executable
chmod +x rsde/rsync

# Check platform
file rsde/rsync

# Check dependencies (Linux)
ldd rsde/rsync
```

## Release Checklist

Before creating a release:

- [ ] All tests pass (`make test`)
- [ ] Code is properly formatted (`make fmt-check`)
- [ ] No clippy warnings (`make clippy`)
- [ ] Documentation is up to date
- [ ] CHANGELOG updated (optional)
- [ ] Version numbers updated in Cargo.toml
- [ ] All changes committed and pushed
- [ ] Tag follows semver format (`vX.Y.Z`)

After creating a release:

- [ ] All build jobs completed successfully
- [ ] Release page shows all artifacts
- [ ] Download and test at least one binary
- [ ] Docker image is pullable
- [ ] Helm chart is installable
- [ ] Release notes are correct

## Version Numbering Guidelines

Follow [Semantic Versioning](https://semver.org/):

- **Major version** (X.0.0): Breaking changes
  - API changes that break compatibility
  - Major feature overhauls
  - Configuration format changes

- **Minor version** (x.Y.0): New features
  - New functionality (backward compatible)
  - New configuration options
  - Performance improvements

- **Patch version** (x.y.Z): Bug fixes
  - Bug fixes
  - Security patches
  - Documentation updates

## Example Release Commands

```bash
# Patch release (bug fix)
git tag v1.0.1 -m "Fix memory leak in file watcher"
git push origin v1.0.1

# Minor release (new feature)
git tag v1.1.0 -m "Add HTTP source support"
git push origin v1.1.0

# Major release (breaking change)
git tag v2.0.0 -m "Redesign configuration format"
git push origin v2.0.0

# Pre-release
git tag v2.0.0-beta.1 -m "Beta release for v2.0.0"
git push origin v2.0.0-beta.1
```

## Automation Details

The release workflow automatically:

1. **Builds** for multiple platforms (Linux/macOS, x86_64/ARM64)
2. **Strips** binaries to reduce size
3. **Creates** structured `rsde/` directory
4. **Generates** VERSION file with build metadata
5. **Compresses** into tarball
6. **Calculates** SHA256 checksums
7. **Uploads** to GitHub Release
8. **Generates** changelog from git commits
9. **Tags** Docker images with version numbers
10. **Updates** Helm chart version
11. **Publishes** to GitHub Container Registry

All of this happens automatically when you push a tag!
