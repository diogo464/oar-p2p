# Build stage
FROM docker.io/alpine:3.22 AS builder

# Install build dependencies
RUN apk add --no-cache \
    rust \
    cargo \
    musl-dev

# Create app directory
WORKDIR /app

# Copy source code
COPY . .

# Build the application in release mode
RUN cargo build --release

# Runtime stage
FROM docker.io/alpine:3.22

# Install runtime dependencies
RUN apk add --no-cache \
    iproute2 \
    iproute2-tc

# Copy the binary from builder stage
COPY --from=builder /app/target/release/oar-p2p /usr/local/bin/oar-p2p

# Set the binary as executable
RUN chmod +x /usr/local/bin/oar-p2p

# Run as non-root user
RUN addgroup -g 1000 appgroup && \
    adduser -D -s /bin/sh -u 1000 -G appgroup appuser

USER appuser

ENTRYPOINT ["/usr/local/bin/oar-p2p"]