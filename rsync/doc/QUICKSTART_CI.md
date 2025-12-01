# Quick Start Guide for CI/CD

## Prerequisites

1. GitHub repository with the code
2. GitHub Actions enabled (default for public repos)
3. Permissions to push to GitHub Container Registry

## Setup Steps

### 1. Enable GitHub Container Registry

The workflow will automatically push Docker images to GitHub Container Registry (ghcr.io). 

**For private repositories**, ensure GitHub Actions has the correct permissions:
1. Go to repository Settings > Actions > General
2. Under "Workflow permissions", select "Read and write permissions"
3. Check "Allow GitHub Actions to create and approve pull requests"

### 2. Push the CI/CD Configuration

```bash
cd /opt/mystorage/github/rsde

# Add the new files
git add .github/
git add rsync/CI_CD.md
git add rsync/Makefile
git add rsync/helm/rsync/templates/tests/

# Commit
git commit -m "Add CI/CD pipeline with unit tests and Helm deployment validation"

# Push to trigger the workflow
git push origin master
```

### 3. Monitor the Workflow

1. Go to your GitHub repository
2. Click on the "Actions" tab
3. You should see the "Rsync CI/CD" workflow running

The workflow will:
- ✅ Run unit tests (`make test`)
- ✅ Validate Helm chart
- ✅ Build Docker image
- ✅ Deploy to Kind cluster
- ✅ Verify deployment works

### 4. Check Results

Each job will show:
- 🟢 Green checkmark if successful
- 🔴 Red X if failed
- Click on any job to see detailed logs

## Workflow Triggers

The CI/CD pipeline runs when:
- You push to `master`, `main`, or `develop` branches
- Someone creates a pull request to these branches
- Changes are made to files in `rsync/` directory

## What Happens on Failure?

### If Unit Tests Fail (`make test`)
- The workflow stops immediately
- No Docker image is built
- No deployment happens
- You'll see the exact test failure in the logs

### If Helm Deployment Fails
- Docker image is still built and pushed
- But the deployment test fails
- Check the logs to see why the pod didn't start

## Local Validation Before Push

Always run these locally before pushing:

```bash
cd rsync

# Run all checks (same as CI)
make check

# This runs:
# - make fmt-check (code formatting)
# - make clippy (linter)
# - make test (unit tests)
```

## Viewing Docker Images

After a successful build, your image will be available at:
```
ghcr.io/<YOUR_GITHUB_USERNAME>/rsde/rsync:latest
```

View all images:
1. Go to your GitHub profile
2. Click "Packages"
3. Find "rsde/rsync"

## Using the Built Image

```bash
# Pull the image
docker pull ghcr.io/<YOUR_GITHUB_USERNAME>/rsde/rsync:latest

# Or use in Kubernetes
helm install rsync ./rsync/helm/rsync \
  --set image.repository=ghcr.io/<YOUR_GITHUB_USERNAME>/rsde/rsync \
  --set image.tag=latest
```

## Troubleshooting

### "Permission denied" when pushing Docker image
- Check repository Settings > Actions > General
- Enable "Read and write permissions"

### Tests pass locally but fail in CI
- Check Rust version (CI uses latest stable)
- Check for hardcoded paths
- Review test logs in GitHub Actions

### Helm deployment times out
- Check pod logs in the CI output
- Verify Docker image was built successfully
- Check resource limits in values.yaml

## Next Steps

After the first successful run:
1. Check the "Packages" section to see your Docker image
2. Download the Helm chart artifact from the workflow run
3. Consider adding badges to your README

### Adding CI Status Badge

Add this to your README.md:
```markdown
![CI/CD](https://github.com/<USERNAME>/<REPO>/actions/workflows/rsync-ci.yml/badge.svg)
```

Replace `<USERNAME>` and `<REPO>` with your values.
