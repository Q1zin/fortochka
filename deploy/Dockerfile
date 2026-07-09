# Сборка: полный Rust-образ; в рантайм уезжает только бинарь.
FROM rust:1.93-slim AS builder
WORKDIR /app
COPY . .
# Кэш-маунты ускоряют повторные локальные сборки (в CI кэшируется через buildx)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release -p fortochka-server \
    && cp target/release/fortochka-server /usr/local/bin/fortochka-server

FROM debian:bookworm-slim
RUN useradd --system --uid 10001 fortochka \
    && mkdir /data && chown fortochka /data
COPY --from=builder /usr/local/bin/fortochka-server /usr/local/bin/fortochka-server
USER fortochka
ENV BIND_ADDR=0.0.0.0:8080 \
    DATA_DIR=/data \
    RUST_LOG=info
EXPOSE 8080
CMD ["fortochka-server"]
