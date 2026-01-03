# Stage 1: Build frontend with Node.js
FROM swr.cn-north-4.myhuaweicloud.com/ddn-k8s/docker.io/node:24.11.1 AS frontend-builder

WORKDIR /build/webserver

# Copy only frontend files
COPY webserver/Makefile .
COPY webserver/frontend/package*.json ./frontend/
COPY webserver/frontend ./frontend

# Build frontend
RUN make build

# Stage 2: Build Rust binaries
FROM swr.cn-north-4.myhuaweicloud.com/ddn-k8s/docker.io/rust:1.92 AS rust-builder

WORKDIR /usr/src/app

COPY . .

# Build all binaries using the Makefile
RUN make build

# Stage 3: Final runtime image


# Stage 3: Final runtime image
FROM swr.cn-north-4.myhuaweicloud.com/ddn-k8s/docker.io/debian:bookworm-slim

# Install necessary runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy all binaries from the rust builder stage
COPY --from=rust-builder /usr/src/app/bin/rsync /usr/local/bin/rsync
COPY --from=rust-builder /usr/src/app/bin/rc /usr/local/bin/rc
COPY --from=rust-builder /usr/src/app/bin/apiserver /usr/local/bin/apiserver

# Copy frontend dist files from the frontend builder stage
COPY --from=frontend-builder /build/webserver/frontend/dist /app/webserver/frontend/dist

# Set the working directory
WORKDIR /app

# Run the apiserver service by default
CMD ["apiserver"]
