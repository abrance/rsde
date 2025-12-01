# Rsync CI/CD Pipeline

This document describes the GitHub Actions CI/CD pipeline for the rsync project.

## Workflow Overview

The CI/CD pipeline (`rsync-ci.yml`) is triggered on:
- Push to `master`, `main`, or `develop` branches
- Pull requests to `master`, `main`, or `develop` branches
- Only when files in `rsync/` directory change

## Pipeline Stages

### 1. Test (Unit Tests)
**Purpose**: Run all unit tests to ensure code quality

**Steps**:
- Checkout code
- Install Rust toolchain with rustfmt and clippy
- Cache cargo dependencies for faster builds
- Run `make fmt-check` - verify code formatting
- Run `make clippy` - run Rust linter
- Run `make test` - execute all unit tests
- Run `make build` - build release binary

**Failure Condition**: If any test fails, the entire pipeline fails

### 2. Helm Lint
**Purpose**: Validate Helm chart syntax and structure

**Steps**:
- Checkout code
- Install Helm
- Run `helm lint` on the chart
- Run `helm template` for dry-run validation

### 3. Docker Build
**Purpose**: Build and push Docker image to GitHub Container Registry

**Triggers**: Only on push events (not PRs)

**Steps**:
- Build Docker image using rsync/Dockerfile
- Push to `ghcr.io/<owner>/<repo>/rsync`
- Tag with branch name, commit SHA, and `latest` (for default branch)

**Permissions Required**: 
- `contents: read`
- `packages: write`

### 4. Helm Test (Deployment Verification)
**Purpose**: Deploy and test the application in a real Kubernetes cluster

**Triggers**: Only on push to `master` branch

**Steps**:
- Create Kind (Kubernetes in Docker) cluster
- Build Docker image
- Load image into Kind cluster
- Deploy using `helm install`
- Wait for pods to be ready
- Check application logs
- Verify output files
- Run helm tests (if defined)
- Cleanup resources

**Failure Condition**: If deployment fails or pods don't become ready within 2 minutes

### 5. Release
**Purpose**: Package Helm chart for distribution

**Triggers**: Only on successful deployment test on `master` branch

**Steps**:
- Package Helm chart
- Upload as artifact (retained for 30 days)

## Environment Variables

- `CARGO_TERM_COLOR`: always (colored output)
- `REGISTRY`: ghcr.io (GitHub Container Registry)
- `IMAGE_NAME`: Dynamic based on repository

## Caching Strategy

To speed up builds, the pipeline caches:
- Cargo registry (`~/.cargo/registry`)
- Cargo git dependencies (`~/.cargo/git`)
- Build artifacts (`rsync/target`)

## Required Secrets

No custom secrets required. Uses built-in `GITHUB_TOKEN` for:
- Pushing to GitHub Container Registry
- Accessing repository

## Local Testing

Before pushing, you can run the same checks locally:

```bash
cd rsync

# Run all checks
make check

# Or individually
make fmt-check
make clippy
make test
make build

# Test Helm chart
helm lint ./helm/rsync
helm template rsync ./helm/rsync --debug
```

## Helm Chart Testing in Kind

To replicate the CI environment locally:

```bash
# Install Kind
go install sigs.k8s.io/kind@latest

# Create cluster
kind create cluster --name rsync-test

# Build and load image
cd rsync
docker build -t rsync:test .
kind load docker-image rsync:test --name rsync-test

# Deploy
helm install rsync ./helm/rsync --set image.tag=test

# Check status
kubectl get pods
kubectl logs -l app.kubernetes.io/name=rsync

# Cleanup
helm uninstall rsync
kind delete cluster --name rsync-test
```

## Troubleshooting

### Test Failures
- Check test output in the GitHub Actions log
- Run `make test` locally to reproduce
- Fix issues and push again

### Docker Build Failures
- Verify Dockerfile is valid
- Check Rust edition compatibility
- Ensure all dependencies are available

### Helm Deployment Failures
- Check pod logs: `kubectl logs -l app.kubernetes.io/name=rsync`
- Verify ConfigMap is created correctly
- Check resource constraints
- Ensure image is available in the cluster

### Permission Errors
- Ensure GitHub Actions has permission to write packages
- Check repository settings under Settings > Actions > General

## Future Enhancements

Potential improvements:
- Add integration tests
- Add security scanning (Trivy, Snyk)
- Add SAST/DAST tools
- Multi-arch Docker builds (ARM64, AMD64)
- Automatic version tagging
- Deploy to staging environment
- Performance benchmarks
- Code coverage reporting
