FROM rust:1.98-alpine AS builder

RUN apk add --no-cache musl-dev gcc g++ make binutils
WORKDIR /usr/src/mcp-meta-indexer

COPY Cargo.toml Cargo.lock* ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release --locked
RUN rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release --locked

RUN strip target/release/mcp-meta-indexer

FROM alpine:latest
RUN apk add --no-cache ripgrep

WORKDIR /app
COPY --from=builder /usr/src/mcp-meta-indexer/target/release/mcp-meta-indexer /usr/local/bin/mcp-meta-indexer

ENV MCP_TRANSPORT=sse
EXPOSE 3000

CMD ["mcp-meta-indexer"]
