# rust:1.97.0-bookworm; Dependabot updates the tag and digest together.
FROM rust:1.97.0-bookworm@sha256:7d0723df719e7f213b69dc7c8c595985c3f4b060cfbee4f7bc0e347a86fe3b6a AS builder

WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates

RUN cargo build --locked --release

# debian:bookworm-slim; Dependabot updates the tag and digest together.
FROM debian:bookworm-slim@sha256:60eac759739651111db372c07be67863818726f754804b8707c90979bda511df AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
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

ENTRYPOINT ["/usr/local/bin/prodex-gateway"]
CMD ["serve", "--listen", "0.0.0.0:4000"]
