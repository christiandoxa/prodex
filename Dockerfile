# rust:1.99.1-bookworm; Dependabot updates the tag and digest together.
FROM rust:1.99.1-bookworm@sha256:9181b56c2a21ab1d5e73cffe04fc762426d30aa7fac3c91141dd033f49a7fc8b AS builder

WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates
COPY examples ./examples
COPY benches ./benches
COPY install.sh install.ps1 ./

RUN cargo build --locked --release

# debian:bookworm-slim; Dependabot updates the tag and digest together.
FROM debian:bookworm-slim@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3 AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
      ca-certificates=20230311+deb12u1 \
      curl=7.88.1-10+deb12u15 \
      libpcre2-8-0=10.42-1+deb12u1 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --uid 10001 prodex \
    && mkdir -p /var/lib/prodex /var/log/prodex \
    && chown -R prodex:prodex /var/lib/prodex /var/log/prodex

COPY --from=builder /workspace/target/release/prodex /usr/local/bin/prodex
COPY --from=builder /workspace/target/release/prodex-gateway /usr/local/bin/prodex-gateway
COPY --from=builder /workspace/target/release/prodex-control-plane /usr/local/bin/prodex-control-plane

ENV PRODEX_HOME=/var/lib/prodex
ENV PRODEX_RUNTIME_LOG_DIR=/var/log/prodex

USER prodex
EXPOSE 4000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -fsS http://127.0.0.1:4000/readyz || exit 1

ENTRYPOINT ["/usr/local/bin/prodex-gateway"]
CMD ["serve", "--listen", "0.0.0.0:4000"]
