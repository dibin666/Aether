# syntax=docker/dockerfile:1
# Aether Gateway runtime image (cross-compilation)
# Binary and frontend assets are pre-built by CI; this Dockerfile only packages them.
#
# Keep the runtime layout compatible with Dockerfile.app.local. This image is
# also used as a drop-in replacement for the VPS image produced by deploy.sh.
# Usage: docker buildx build --platform linux/amd64,linux/arm64 -f Dockerfile.app .
#
# Build context must contain:
#   dist/aether-gateway-amd64   (x86_64-unknown-linux-musl cross-compiled binary)
#   dist/aether-gateway-arm64   (aarch64-unknown-linux-musl cross-compiled binary)
#   dist/frontend/              (npm run build output)

# --- layout stage: create the same layout as the local-build image ---
# distroless has no shell, so use busybox for the filesystem setup and checks.
FROM busybox:1.37-musl AS layout

ARG TARGETARCH

RUN mkdir -p /runtime-root/app/logs /runtime-root/srv/frontend /runtime-root/usr/local/bin

COPY dist/aether-gateway-${TARGETARCH} /runtime-root/usr/local/bin/aether-gateway
RUN chmod 0755 /runtime-root/usr/local/bin/aether-gateway
COPY dist/frontend/ /runtime-root/srv/frontend/

# Fail the image build when the CI artifact layout is incomplete. Without an
# index.html the gateway starts successfully but every WebUI route returns 404.
RUN test -x /runtime-root/usr/local/bin/aether-gateway \
    && test -s /runtime-root/srv/frontend/index.html

# --- final stage: distroless runtime ---
FROM gcr.io/distroless/static-debian12

COPY --from=layout /runtime-root/ /

WORKDIR /app

ENV LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    RUST_LOG=aether_gateway=info \
    APP_PORT=8084 \
    AETHER_UPDATE_STRATEGY=manual \
    AETHER_GATEWAY_STATIC_DIR=/srv/frontend

EXPOSE 8084

HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD ["/usr/local/bin/aether-gateway", "--healthcheck"]

USER root
ENTRYPOINT ["/usr/local/bin/aether-gateway"]
