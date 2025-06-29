IMAGE_TAG := "ghcr.io/diogo464/oar-p2p:latest"

default:
    just --list

# Build the container image
build:
    docker build -f Containerfile -t {{IMAGE_TAG}} .

# Build and push the container image
push: build
    docker push {{IMAGE_TAG}}

python *args:
    scp oar_p2p_net.py cluster:./
    ssh cluster /home/diogo464/.local/bin/uv run python ./oar_p2p_net.py {{args}}
