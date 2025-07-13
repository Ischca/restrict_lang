# Multi-stage Docker build for Restrict Language
FROM rust:1.75-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    curl \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy Cargo files for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY warder/Cargo.toml warder/
COPY src/lib.rs src/
COPY warder/src/main.rs warder/src/

# Build dependencies first (cached layer)
RUN cargo build --release --lib
RUN cd warder && cargo build --release --lib || true
RUN rm src/lib.rs warder/src/main.rs

# Copy all source code
COPY src/ src/
COPY warder/src/ warder/src/
COPY tests/ tests/
COPY examples/ examples/
COPY std/ std/

# Build the full project
RUN cargo build --release
RUN cd warder && cargo build --release

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
COPY --from=builder /app/warder/target/release/warder /usr/local/bin/
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
LABEL version="0.1.0"
LABEL description="Restrict Language Compiler and Warder Package Manager for WebAssembly"