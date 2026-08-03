# Multi-stage Docker build for Restrict Language
FROM rust:bookworm AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy the workspace after .dockerignore removes local build artifacts
COPY . .

# Build the full project
RUN cargo build --workspace --locked --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Install wasmtime for running WASM output
RUN curl https://wasmtime.dev/install.sh -sSf | bash

# Create app user
RUN useradd -r -s /bin/false appuser

# Copy binaries from builder stage
COPY --from=builder /app/target/release/restrict_lang /usr/local/bin/
COPY --from=builder /app/target/release/warder /usr/local/bin/
COPY --from=builder /app/std/ /usr/local/share/restrict_lang/std/

# Set up environment
ENV PATH="/root/.wasmtime/bin:${PATH}"
ENV RESTRICT_LANG_STD_PATH="/usr/local/share/restrict_lang/std"

# Create working directory
WORKDIR /workspace
RUN chown appuser:appuser /workspace

# Switch to app user
USER appuser

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD restrict_lang --version && warder --version || exit 1

# Default command
CMD ["restrict_lang", "--help"]

# Labels
LABEL maintainer="Restrict Language Team"
LABEL version="0.0.1"
LABEL description="Restrict Language Compiler and Warder Package Manager for WebAssembly"
