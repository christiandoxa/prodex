# uv:0.13.9; Dependabot updates the tag and digest together.
FROM ghcr.io/astral-sh/uv:0.13.9@sha256:d3b601f0c7b118bc1753febf7c41cc80981df678445e0254284eecf84db79a7d AS uv

# rust:1.100.0-trixie; Dependabot updates the tag and digest together.
FROM rust:1.100.0-trixie@sha256:223cc32ddca8f3ee90c436dbc94b6b5f052ea7f9653c5201e9612f08ad7d21b8 AS builder

COPY --from=uv /uv /uvx /usr/local/bin/
RUN uv venv --managed-python --python 3.15.2 /opt/mojo \
    && uv pip install --no-cache-dir --python /opt/mojo/bin/python --prerelease=disallow mojo==1.2.0 \
    && /opt/mojo/bin/mojo --version

ENV PATH="/opt/mojo/bin:${PATH}"
ENV PRODEX_MOJO_REQUIRED=1
ENV PRODEX_MOJO_VERSION=1.2.0

WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY crates ./crates
COPY vendor ./vendor
COPY mojo ./mojo
COPY examples ./examples
COPY benches ./benches
COPY install.sh install.ps1 ./

ARG CARGO_BUILD_JOBS=4
RUN cargo build --locked --release

# debian:trixie-slim; Dependabot updates the tag and digest together.
FROM debian:trixie-slim@sha256:a72b4433de1cf7d6bae0c4cee74d0547f58601a643b3375f070c51600d63b6b2 AS runtime

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
      ca-certificates=20250419+deb13u1 \
      curl=8.14.1-2+deb13u7 \
      libpcre2-8-0=10.46-1~deb13u3 \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --uid 10001 prodex \
    && mkdir -p /var/lib/prodex /var/log/prodex \
    && chown -R prodex:prodex /var/lib/prodex /var/log/prodex

COPY --from=builder /workspace/target/release/prodex /usr/local/bin/prodex

ENV PRODEX_HOME=/var/lib/prodex
ENV PRODEX_RUNTIME_LOG_DIR=/var/log/prodex

USER prodex
# This image runs a CLI command; it does not start an HTTP service by default.
HEALTHCHECK NONE

ENTRYPOINT ["/usr/local/bin/prodex"]
CMD ["--help"]
