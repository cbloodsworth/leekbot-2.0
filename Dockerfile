### Building and installation
FROM rust:1-bullseye AS builder

# Build and runtime dependencies (split into two stages later?)
RUN apt-get update \
 && apt-get -y install libsqlite3-dev \
 && rm /var/lib/apt/lists/* -fr

# Install leekbot
WORKDIR /usr/src/leekbot
COPY . .
RUN cargo install --path .

CMD ["/usr/local/cargo/bin/leekbot"]
