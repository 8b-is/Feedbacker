# 🚢 Feedbacker Dockerfile - Multi-stage build for maximum efficiency! 🚢
# Built with love by Aye & Hue - Making deployments as smooth as sailing! ⛵
# Stage 1: Build environment

FROM rust:1.83-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libpq-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first for better layer caching
COPY Cargo.toml Cargo.lock ./

# Create dummy main to build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src
COPY examples ./examples

# Touch main.rs to invalidate the cache for it
RUN touch src/main.rs

# Build the actual application
RUN cargo build --release --bin feedbacker

# Stage 2: Runtime environment
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libpq5 \
    openssh-client \
    git \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user for security
RUN useradd -ms /bin/bash feedbacker

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/feedbacker /app/feedbacker

# Create necessary directories
RUN mkdir -p /app/logs /home/feedbacker/.ssh && \
    chown -R feedbacker:feedbacker /app /home/feedbacker/.ssh && \
    chmod 700 /home/feedbacker/.ssh

# Switch to non-root user
USER feedbacker

# Expose the service port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=10s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/api/health || exit 1

# Set environment defaults
ENV RUST_LOG=info,feedbacker=debug
ENV SERVER_ADDRESS=0.0.0.0:3000
ENV ENVIRONMENT=production

# Run the application
CMD ["/app/feedbacker"]
