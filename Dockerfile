### Building and installation
FROM rust:1-bullseye AS builder

WORKDIR /usr/src/leekbot
COPY . .
RUN cargo install --path .

### Runtime image
FROM debian:bullseye-slim

RUN apt-get update && \
    rm /var/lib/apt/lists/* -fr
COPY --from=builder /usr/local/cargo/bin/leekbot /usr/local/bin/leekbot

CMD ["leekbot"]
