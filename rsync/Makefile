.PHONY: help build test clean run docker-build docker-push helm-install helm-upgrade helm-uninstall

# Default target
help:
	@echo "Available targets:"
	@echo "  build           - Build the release binary"
	@echo "  test            - Run all unit tests"
	@echo "  test-verbose    - Run all unit tests with verbose output"
	@echo "  clean           - Clean build artifacts"
	@echo "  run             - Run the application locally"
	@echo "  docker-build    - Build Docker image"
	@echo "  docker-push     - Push Docker image to registry"
	@echo "  helm-install    - Install application via Helm"
	@echo "  helm-upgrade    - Upgrade application via Helm"
	@echo "  helm-uninstall  - Uninstall application via Helm"

# Build the release binary
build:
	cargo build --release

# Run all unit tests
test:
	cargo test --workspace --all-features

# Run tests with verbose output
test-verbose:
	cargo test --workspace --all-features -- --nocapture --test-threads=1

# Run tests for a specific package
test-rule:
	cargo test -p rule --all-features

test-core:
	cargo test -p core --all-features

# Clean build artifacts
clean:
	cargo clean
	rm -rf target/

# Run the application
run:
	cargo run

# Run with config directory
run-with-config:
	RSYNC_CONFIG_DIR=. cargo run

# Build Docker image
docker-build:
	docker build -t rsync:latest -f ../Dockerfile .

# Tag and push Docker image
docker-push: docker-build
	docker tag rsync:latest $(REGISTRY)/rsync:$(VERSION)
	docker push $(REGISTRY)/rsync:$(VERSION)

# Import Docker image to k3s
docker-import-k3s: docker-build
	docker save rsync:latest | sudo k3s ctr images import -

# Helm commands
helm-install:
	helm install rsync ./helm/rsync

helm-upgrade:
	helm upgrade rsync ./helm/rsync

helm-uninstall:
	helm uninstall rsync

# Watch for changes and run tests
watch-test:
	cargo watch -x "test --workspace --all-features"

# Format code
fmt:
	cargo fmt --all

# Check code formatting
fmt-check:
	cargo fmt --all -- --check

# Run clippy linter
clippy:
	cargo clippy --workspace --all-features

# Check everything (format, clippy, test)
check: fmt-check clippy test

# Development workflow - format and test
dev: fmt test
