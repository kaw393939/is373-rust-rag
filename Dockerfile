# syntax=docker/dockerfile:1
FROM node:22.22.2-bookworm-slim AS frontend
WORKDIR /build/frontend
COPY frontend/package*.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

FROM rust:1.94.0-bookworm AS backend
ARG TARGETARCH
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src/ ./src/
RUN --mount=type=cache,id=is373-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=is373-target-${TARGETARCH},target=/build/target \
    cargo build --release --locked && cp target/release/is373-rag /build/is373-rag

FROM debian:bookworm-slim AS speech
ARG WHISPER_COMMIT=d1be6fde11ac6e0407606b4e42fe72d34add8037
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl cmake build-essential \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /build
RUN curl -fL --retry 3 "https://github.com/ggml-org/whisper.cpp/archive/${WHISPER_COMMIT}.tar.gz" -o source.tar.gz \
    && tar xzf source.tar.gz --strip-components=1 \
    && cmake -S . -B build -DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF \
       -DGGML_NATIVE=OFF -DGGML_OPENMP=OFF -DGGML_BLAS=OFF -DWHISPER_BUILD_TESTS=OFF \
    && cmake --build build --target whisper-cli --parallel 2

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl ffmpeg poppler-utils libstdc++6 \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 10001 ordo && useradd --uid 10001 --gid ordo --create-home ordo \
    && mkdir -p /app/data/models && chown -R ordo:ordo /app
WORKDIR /app
COPY --from=backend /build/is373-rag /usr/local/bin/is373-rag
COPY --from=speech /build/build/bin/whisper-cli /usr/local/bin/whisper-cli
COPY --from=frontend /build/frontend/dist ./frontend/dist
COPY corpus/ ./corpus/
COPY scripts/download_whisper.sh scripts/container-entrypoint.sh ./scripts/
ENV BIND_ADDRESS=0.0.0.0 PORT=3730 DATABASE_PATH=/app/data/rag.sqlite \
    WHISPER_MODEL=/app/data/models/ggml-small.bin WHISPER_CLI=whisper-cli
USER 10001:10001
EXPOSE 3730
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${PORT}/api/health" || exit 1
ENTRYPOINT ["bash", "/app/scripts/container-entrypoint.sh"]
CMD ["serve"]
