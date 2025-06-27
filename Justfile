IMAGE_TAG := "ghcr.io/diogo464/oar-p2p:latest"

# Build the container image
build:
    docker build -f Containerfile -t {{IMAGE_TAG}} .

# Build and push the container image
push: build
    docker push {{IMAGE_TAG}}