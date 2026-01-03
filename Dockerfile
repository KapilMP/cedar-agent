# Build stage
FROM rust:latest AS builder

WORKDIR /app

# Copy project files
COPY Cargo.toml ./
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y curl ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary
COPY --from=builder /app/target/release/cedar-agent /usr/local/bin/cedar-agent

# Create directory for policies
RUN mkdir -p /app/policies

WORKDIR /app

# Expose port
EXPOSE 8181

# Set environment variables
ENV CEDAR_POLICY_PATH=/app/policies/policy.cedar
ENV CEDAR_SCHEMA_PATH=/app/policies/schema.cedarschema.json
ENV BIND_ADDR=0.0.0.0:8181
ENV RUST_LOG=info

# Run the Cedar agent
CMD ["cedar-agent"]