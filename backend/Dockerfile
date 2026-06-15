# ── build ──
FROM rust:1-slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY shared ./shared
COPY backend ./backend
# frontend(wasm) · cli 는 백엔드 이미지에서 제외 — 임시 빈 멤버로 대체.
RUN mkdir -p frontend/src cli/src && \
    printf '[package]\nname = "frontend"\nversion = "0.1.0"\nedition = "2021"\n' > frontend/Cargo.toml && \
    echo 'fn main() {}' > frontend/src/main.rs && \
    printf '[package]\nname = "cli"\nversion = "0.1.0"\nedition = "2021"\n' > cli/Cargo.toml && \
    echo 'fn main() {}' > cli/src/main.rs
RUN cargo build --release -p backend

# ── runtime ──
FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/backend /usr/local/bin/backend
ENV PORT=8080
EXPOSE 8080
CMD ["backend"]
