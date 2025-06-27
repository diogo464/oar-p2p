IMAGE_TAG := "ghcr.io/diogo464/oar-p2p:latest"

default:
    just --list

# Build the container image
build:
    docker build -f Containerfile -t {{IMAGE_TAG}} .

# Build and push the container image
push: build
    docker push {{IMAGE_TAG}}

cluster:
    RUSTFLAGS='-C link-arg=-s' cargo build --target x86_64-unknown-linux-musl --target-dir target/
    scp target/x86_64-unknown-linux-musl/debug/oar-p2p cluster:./
