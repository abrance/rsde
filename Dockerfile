FROM rust:1.83 AS builder

WORKDIR /usr/src/app
COPY . .

# Build only the rsync package
RUN cargo build --release -p rsync

FROM debian:bookworm-slim

# Install necessary runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /usr/src/app/target/release/rsync /usr/local/bin/rsync

# Set the working directory
WORKDIR /app

# Run the application
CMD ["rsync"]
