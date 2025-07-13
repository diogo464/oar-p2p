#!/bin/bash

# Script to generate a schedule based on `cargo run -- net show` output
# Usage: ./generate-schedule.sh [image]

set -e

# Default values
IMAGE="${1:-ghcr.io/diogo464/oar-p2p/demo:latest}"

addresses_output=$(oar-p2p net show)
address_count=$(echo "$addresses_output" | wc -l)

echo "Generating schedule with $address_count containers..." >&2
echo "Using image: $IMAGE" >&2

# Start JSON array
echo "["

# Process each address
first=true
while IFS=' ' read -r machine address; do
    # Skip empty lines
    if [ -z "$address" ]; then
        continue
    fi
    
    # Add comma separator for all but first entry
    if [ "$first" = true ]; then
        first=false
    else
        echo ","
    fi
    
    # Generate container entry directly with proper escaping
    printf '    {\n        "address": "%s",\n        "image": "%s",\n        "env": {\n            "ADDRESS": "%s",\n            "MACHINE": "%s",\n            "MESSAGE": "Container on %s with address %s"\n        }\n    }' \
        "$address" "$IMAGE" "$address" "$machine" "$machine" "$address"
    
done <<< "$addresses_output"

# Close JSON array
echo ""
echo "]"
