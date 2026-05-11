# ---- Builder stage ----
FROM rust:1.92-slim AS builder

WORKDIR /app

# System dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

# Install trunk and wasm target for frontend build
RUN rustup target add wasm32-unknown-unknown && \
    cargo install trunk --version 0.21.14

# Copy workspace Cargo files first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY backend/Cargo.toml backend/
COPY frontend/Cargo.toml frontend/

# Copy source
COPY backend/src backend/src/
COPY frontend/src frontend/src/
COPY frontend/index.html frontend/
COPY frontend/Trunk.toml frontend/
COPY frontend/tailwind.config.js frontend/

# Build frontend with trunk (produces frontend/dist/)
WORKDIR /app/frontend
RUN trunk build --release

# Build backend release binary
WORKDIR /app
RUN cargo build --release --bin volsync-webui-backend

# ---- Final stage ----
FROM debian:bookworm-slim AS runner

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Create public/ directory for static frontend files
RUN mkdir -p public

# Copy backend binary and built frontend files
COPY --from=builder /app/target/release/volsync-webui-backend /app/volsync-webui
COPY --from=builder /app/frontend/dist /app/public

EXPOSE 8080

ENTRYPOINT ["/app/volsync-webui"]
