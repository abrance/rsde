.PHONY: help build clean test rsync rc webserver apiserver release release-linux-x86_64 release-linux-aarch64 release-darwin-x86_64 release-darwin-aarch64

# Default target
help:
	@echo "Available targets:"
	@echo "  build                 - Build all binaries (rsync, rc, webserver, apiserver)"
	@echo "  rsync                 - Build only rsync binary"
	@echo "  rc                    - Build only rc binary"
	@echo "  webserver             - Build only webserver binary"
	@echo "  apiserver             - Build only apiserver binary"
	@echo "  release               - Build release binaries for all platforms"
	@echo "  release-linux-x86_64  - Build release binaries for Linux x86_64"
	@echo "  release-linux-aarch64 - Build release binaries for Linux aarch64"
	@echo "  release-darwin-x86_64 - Build release binaries for Darwin x86_64"
	@echo "  release-darwin-aarch64 - Build release binaries for Darwin aarch64"
	@echo "  clean                 - Clean build artifacts"
	@echo "  test                  - Run tests for all projects"

# Build all binaries
build: rsync rc webserver apiserver

# Build rsync binary
rsync:
	@echo "Building rsync..."
	cargo build --release -p rsync
	mkdir -p bin
	cp target/release/rsync bin/

# Build rc binary
rc:
	@echo "Building rc..."
	cargo build --release -p rc
	mkdir -p bin
	cp target/release/rc bin/

# Build webserver binary
webserver:
	@echo "Building webserver..."
	cargo build --release -p webserver
	mkdir -p bin
	cp target/release/webserver bin/

# Build apiserver binary
apiserver:
	@echo "Building apiserver..."
	cargo build --release -p apiserver
	mkdir -p bin
	cp target/release/apiserver bin/

# Build release binaries for all platforms
release: release-linux-x86_64 release-linux-aarch64 release-darwin-x86_64 release-darwin-aarch64

# Build release binaries for Linux x86_64
release-linux-x86_64:
	@echo "Building release binaries for Linux x86_64..."
	CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=x86_64-linux-gnu-gcc \
		cargo build --release --target x86_64-unknown-linux-gnu -p rsync -p rc -p webserver -p apiserver
	mkdir -p bin
	cp target/x86_64-unknown-linux-gnu/release/rsync bin/rsync-linux-x86_64
	cp target/x86_64-unknown-linux-gnu/release/rc bin/rc-linux-x86_64
	cp target/x86_64-unknown-linux-gnu/release/webserver bin/webserver-linux-x86_64
	cp target/x86_64-unknown-linux-gnu/release/apiserver bin/apiserver-linux-x86_64

# Build release binaries for Linux aarch64
release-linux-aarch64:
	@echo "Building release binaries for Linux aarch64..."
	CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
		cargo build --release --target aarch64-unknown-linux-gnu -p rsync -p rc -p webserver -p apiserver
	mkdir -p bin
	cp target/aarch64-unknown-linux-gnu/release/rsync bin/rsync-linux-aarch64
	cp target/aarch64-unknown-linux-gnu/release/rc bin/rc-linux-aarch64
	cp target/aarch64-unknown-linux-gnu/release/webserver bin/webserver-linux-aarch64
	cp target/aarch64-unknown-linux-gnu/release/apiserver bin/apiserver-linux-aarch64

# Build release binaries for Darwin x86_64
release-darwin-x86_64:
	@echo "Building release binaries for Darwin x86_64..."
	cargo build --release --target x86_64-apple-darwin -p rsync -p rc -p webserver -p apiserver
	mkdir -p bin
	cp target/x86_64-apple-darwin/release/rsync bin/rsync-darwin-x86_64
	cp target/x86_64-apple-darwin/release/rc bin/rc-darwin-x86_64
	cp target/x86_64-apple-darwin/release/webserver bin/webserver-darwin-x86_64
	cp target/x86_64-apple-darwin/release/apiserver bin/apiserver-darwin-x86_64

# Build release binaries for Darwin aarch64
release-darwin-aarch64:
	@echo "Building release binaries for Darwin aarch64..."
	cargo build --release --target aarch64-apple-darwin -p rsync -p rc -p webserver -p apiserver
	mkdir -p bin
	cp target/aarch64-apple-darwin/release/rsync bin/rsync-darwin-aarch64
	cp target/aarch64-apple-darwin/release/rc bin/rc-darwin-aarch64
	cp target/aarch64-apple-darwin/release/webserver bin/webserver-darwin-aarch64
	cp target/aarch64-apple-darwin/release/apiserver bin/apiserver-darwin-aarch64

# Clean build artifacts
clean:
	cargo clean
	rm -rf bin/

# Run tests for all projects
test:
	cargo test --workspace

# Quick test to verify binaries
test-binaries:
	@echo "Testing rsync binary..."
	bin/rsync --help || echo "rsync binary not found or failed"
	@echo "Testing rc binary..."
	bin/rc --help || echo "rc binary not found or failed"
	@echo "Testing webserver binary..."
	bin/webserver --help || echo "webserver binary not found or failed"
	@echo "Testing apiserver binary..."
	bin/apiserver --help || echo "apiserver binary not found or failed"

.PHONY: fmt-check
fmt-check:
	cargo fmt -- --check

.PHONY: fmt
fmt:
	cargo fmt

.PHONY: clippy
clippy:
	cargo clippy --all-targets --all-features