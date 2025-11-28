FROM rust:1.83 as builder

WORKDIR /usr/src/rsync
COPY . .

# Build the application
RUN cargo build --release

FROM debian:bookworm-slim

# Install necessary runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /usr/src/rsync/target/release/rsync /usr/local/bin/rsync

# Set the working directory
WORKDIR /app

# Run the application
CMD ["rsync"]
