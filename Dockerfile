# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS builder

WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates

RUN cargo build --locked --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --uid 10001 prodex \
    && mkdir -p /var/lib/prodex /var/log/prodex \
    && chown -R prodex:prodex /var/lib/prodex /var/log/prodex

COPY --from=builder /workspace/target/release/prodex /usr/local/bin/prodex

ENV PRODEX_HOME=/var/lib/prodex
ENV PRODEX_RUNTIME_LOG_DIR=/var/log/prodex

USER prodex
EXPOSE 4000

ENTRYPOINT ["/usr/local/bin/prodex"]
CMD ["gateway", "--listen", "0.0.0.0:4000"]
