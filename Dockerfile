# rust:1.97.1-bookworm; Dependabot updates the tag and digest together.
FROM rust:1.97.1-bookworm@sha256:77fac8b98f9f46062bb680b6d25d5bcaabfc400143952ebc572e924bcbedc3fa AS builder

WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates
COPY examples ./examples
COPY benches ./benches
COPY install.sh install.ps1 THIRD_PARTY_NOTICES.md ./

RUN cargo build --locked --release

# debian:bookworm-slim; Dependabot updates the tag and digest together.
FROM debian:bookworm-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818 AS runtime

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
