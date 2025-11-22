# ==============================================================================
# STAGE 1: The Builder (Heavy)
# ==============================================================================
FROM rustlang/rust:nightly-slim as builder

WORKDIR /app

# 1. Install system dependencies (OpenSSL & pkg-config)
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# 2. Copy manifests first to cache dependencies
COPY Cargo.toml Cargo.lock ./

# 3. Create a dummy main.rs to trigger a build of *just* the dependencies
#    This creates a cached Docker layer for the compiled crates.
RUN mkdir src && \
    echo "fn main() {println!(\"if you see this, the build broke\")}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# 4. Now copy the REAL source code
COPY . .

# 5. SQLx Offline Mode (CRITICAL STEP - Security for Build)
ENV SQLX_OFFLINE=true

# 6. Build the actual binary
RUN touch src/main.rs && cargo build --release

# ==============================================================================
# STAGE 2: The Runner (Lightweight)
# ==============================================================================
FROM debian:bookworm-slim

WORKDIR /app

# 1. Install Runtime Dependencies
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*

# 2. Copy the binary from the builder
COPY --from=builder /app/target/release/fyp-portal .

# 3. Configuration
# CRITICAL FIX: The explicit 'ENV APP_ENV=production' has been REMOVED.
# This prevents an attacker from setting APP_ENV=local at runtime to bypass auth.
EXPOSE 3000

# 4. Run it
CMD ["./fyp-portal"]
