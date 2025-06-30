# OAR P2P Net

A Python tool for managing P2P network configurations in OAR cluster environments with configurable latency matrices.

## Installation

This project uses [uv](https://docs.astral.sh/uv/) for dependency management.

```bash
# Install uv if you haven't already
curl -LsSf https://astral.sh/uv/install.sh | sh

# Run the tool directly
uv run oar_p2p_net.py --help
```

## Usage

The tool provides three main commands:

### Setup Network (`up`)
Configure network interfaces and apply latency settings:
```bash
uv run oar_p2p_net.py up --job-id <job_id> --num-addresses <addresses> --latency-matrix <latency_matrix_file>
```

### Cleanup Network (`down`)
Remove network configurations:
```bash
uv run oar_p2p_net.py down --job-id <job_id>
```

### Generate Configurations (`configurations`)
Preview the network configurations that would be applied:
```bash
uv run oar_p2p_net.py configurations --job-id <job_id> --num-addresses <addresses> --latency-matrix <latency_matrix_file>
```

## Requirements

- Python 3.12+
- Access to OAR cluster environment
- Docker with networking privileges
- Custom networking container: `ghcr.io/diogo464/oar-p2p-networking:latest`

## License

MIT