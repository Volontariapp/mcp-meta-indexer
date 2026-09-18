FROM rust:1.77-alpine as builder

RUN apk add --no-cache musl-dev gcc g++ make
WORKDIR /usr/src/mcp-meta-indexer

COPY Cargo.toml Cargo.lock* ./
# Build dummy pour cacher les dépendances
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release
RUN rm -rf src

COPY src ./src
RUN cargo build --release

# Étape 2 : Runtime
FROM alpine:latest
RUN apk add --no-cache ripgrep

WORKDIR /app
COPY --from=builder /usr/src/mcp-meta-indexer/target/release/mcp-meta-indexer /usr/local/bin/mcp-meta-indexer

ENV MCP_TRANSPORT=sse
EXPOSE 3000

CMD ["mcp-meta-indexer"]
