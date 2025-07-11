demo-build-push:
    docker build -t ghcr.io/diogo464/oar-p2p/demo:latest -f demo.containerfile .
    docker push ghcr.io/diogo464/oar-p2p/demo:latest
