FROM rust:1.84 AS builder

WORKDIR /usr/src/app
COPY . .

# Build all binaries using the Makefile
RUN make build

FROM debian:bookworm-slim

# Install necessary runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy all binaries from the builder stage
COPY --from=builder /usr/src/app/bin/rsync /usr/local/bin/rsync
COPY --from=builder /usr/src/app/bin/rc /usr/local/bin/rc
COPY --from=builder /usr/src/app/bin/webserver /usr/local/bin/webserver
COPY --from=builder /usr/src/app/bin/apiserver /usr/local/bin/apiserver

# Set the working directory
WORKDIR /app

# Run the rsync service by default
CMD ["rsync"]
