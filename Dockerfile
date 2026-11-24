# syntax=docker/dockerfile:1

# ---- build stage ----
FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked --bin dwg-mcp \
 && cp target/release/dwg-mcp /usr/local/bin/dwg-mcp

# ---- runtime stage ----
FROM debian:bookworm-slim
# DejaVu Sans draws Latin and Cyrillic labels; CA certificates are for https downloads.
RUN apt-get update \
 && apt-get install -y --no-install-recommends fonts-dejavu-core ca-certificates \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --home-dir /cache dwg \
 && mkdir -p /cache /work \
 && chown dwg /cache
ENV DWG2PNG_FONT=/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf \
    DWG_MCP_CACHE_DIR=/cache
COPY --from=build /usr/local/bin/dwg-mcp /usr/local/bin/dwg-mcp
USER dwg
# Downloads and results live here for 7 days; mount a volume to keep them between runs.
VOLUME /cache
WORKDIR /work
# MCP over stdio: run with `docker run -i`.
ENTRYPOINT ["dwg-mcp"]
