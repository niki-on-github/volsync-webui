# Development

## Prerequisites

- Nix (for development shelll)

## Nix Dev Shell

```bash
cd volsync-webui
nix develop --accept-flake-config
```

## Build Commands

```bash
# Backend
cargo check -p volsync-webui-backend
cargo build -p volsync-webui-backend

# Frontend
cd frontend && npm install && npx tsc -b && npx vite build

# Format code
cargo fmt
```

## Frontend Dev Server

```bash
cd frontend
npm install
npm run dev
```

The Vite dev server runs on `http://localhost:5173` and proxies API calls to the backend running on port 8080. Make sure the backend is running separately:

```bash
cargo run -p volsync-webui-backend
```
