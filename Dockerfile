# ---- Builder stage ----
FROM rust:slim-bookworm AS builder

WORKDIR /app

# System dependencies
RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libssl-dev \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

# Copy frontend source for layer caching
COPY frontend/package.json frontend/package-lock.json frontend/

# Build frontend with Vite (produces frontend/dist/)
WORKDIR /app/frontend
RUN npm ci
COPY frontend/ .
RUN npx tsc -b && npx vite build

# Build backend release binary
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY backend/Cargo.toml backend/
COPY backend/src backend/src/
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

ENV RUST_LOG=info

ENTRYPOINT ["/app/volsync-webui"]
