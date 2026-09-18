FROM rust:1.80-alpine AS builder

RUN apk add --no-cache musl-dev gcc g++ make binutils
WORKDIR /usr/src/mcp-meta-indexer

# Cache strict des dépendances (Lockfile)
COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release --locked
RUN rm -rf src

# Compilation du vrai code
COPY src ./src
# Touch force Cargo à détecter la modification même si le timestamp Git est plus ancien
RUN touch src/main.rs && cargo build --release --locked

# Strip des symboles de debug pour réduire radicalement la taille de l'image
RUN strip target/release/mcp-meta-indexer

# Étape 2 : Runtime
FROM alpine:latest
RUN apk add --no-cache ripgrep

WORKDIR /app
COPY --from=builder /usr/src/mcp-meta-indexer/target/release/mcp-meta-indexer /usr/local/bin/mcp-meta-indexer

ENV MCP_TRANSPORT=sse
EXPOSE 3000

CMD ["mcp-meta-indexer"]
