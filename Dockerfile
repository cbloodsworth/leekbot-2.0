# ---- Build Stage ----
FROM rust:1-bullseye AS builder

# Install build dependencies
RUN apt-get update \
 && apt-get -y install git libsqlite3-dev \
 && rm -rf /var/lib/apt/lists/*

# Set up workspace
WORKDIR /usr/src/leekbot
COPY . .

# Build the leekbot binary
RUN cargo install --path . --root /usr/local

RUN git log -1 --pretty=%B > /usr/src/leekbot/commit.txt

# ---- Runtime Stage ----
FROM debian:bullseye-slim AS runtime

# Install runtime dependencies
RUN apt-get update \
 && apt-get -y install libsqlite3-0 \
 && apt -y install ca-certificates \
 && rm -rf /var/lib/apt/lists/*

# Create a directory for runtime files
WORKDIR /app

# Copy the built binary from the builder stage
COPY --from=builder /usr/local/bin/leekbot /usr/local/bin/leekbot
COPY --from=builder /usr/src/leekbot/commit.txt /app/commit.txt

# Set the entrypoint
CMD ["/usr/local/bin/leekbot"]
