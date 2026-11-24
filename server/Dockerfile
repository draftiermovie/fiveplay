# FivePlay dedicated server. Build context: the repository root.
#   docker build -f server/Dockerfile -t fiveplay-server .
# The server only depends on the standard library and the shared `net`
# crate, so this builds in well under a minute (no Bevy).
FROM rust:1-slim AS build
WORKDIR /src
COPY net ./net
COPY server ./server
RUN cargo build --release --manifest-path server/Cargo.toml \
    && cp server/target/release/fiveplay-server /fiveplay-server

FROM debian:bookworm-slim
RUN useradd --system --no-create-home fiveplay
COPY --from=build /fiveplay-server /usr/local/bin/fiveplay-server
USER fiveplay
ENV FIVEPLAY_PORT=7777 \
    FIVEPLAY_STATUS_PORT=8080 \
    FIVEPLAY_ROTATION=sector05:ffa,skyreach:ffa \
    FIVEPLAY_MATCH_SECONDS=300 \
    FIVEPLAY_MAX_PLAYERS=6 \
    FIVEPLAY_THREADS=2
# 7777/udp: game traffic. 8080/tcp: JSON status page and health check.
EXPOSE 7777/udp 8080/tcp
HEALTHCHECK --interval=15s --timeout=3s --start-period=5s --retries=3 \
    CMD ["fiveplay-server", "--health"]
CMD ["fiveplay-server"]
