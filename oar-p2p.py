import asyncio
import argparse
import json

MACHINE_INTERFACES = {
    "alakazam-01": None,
    "alakazam-02": None,
    "alakazam-03": None,
    "alakazam-04": None,
    "alakazam-05": None,
    "alakazam-06": None,
    "alakazam-07": None,
    "alakazam-08": None,
    "bulbasaur-1": None,
    "bulbasaur-2": None,
    "bulbasaur-3": None,
    "charmander-1": "bond0",
    "charmander-2": "bond0",
    "charmander-3": "bond0",
    "charmander-4": "bond0",
    "charmander-5": "bond0",
    "gengar-1": None,
    "gengar-2": None,
    "gengar-3": None,
    "gengar-4": None,
    "gengar-5": None,
    "kadabra-01": None,
    "kadabra-02": None,
    "kadabra-03": None,
    "kadabra-04": None,
    "kadabra-05": None,
    "kadabra-06": None,
    "kadabra-07": None,
    "kadabra-08": None,
    "lugia-1": None,
    "lugia-2": None,
    "lugia-3": None,
    "lugia-4": None,
    "lugia-5": None,
    "magikarp-1": None,
    "moltres-01": None,
    "moltres-02": None,
    "moltres-03": None,
    "moltres-04": None,
    "moltres-05": None,
    "moltres-06": None,
    "moltres-07": None,
    "moltres-08": None,
    "moltres-09": None,
    "moltres-10": None,
    "oddish-1": None,
    "psyduck-1": None,
    "psyduck-2": None,
    "psyduck-3": None,
    "shelder-1": None,
    "squirtle-1": None,
    "squirtle-2": None,
    "squirtle-3": None,
    "squirtle-4": None,
    "staryu-1": None,
    "sudowoodo-1": None,
    "vulpix-1": None,
}


class LatencyMatrix:
    def __init__(self, matrix: list[list[float]]):
        # Assert matrix is not None
        assert matrix is not None, "Matrix cannot be None"
        # Assert matrix is square
        size = len(matrix)
        for row in matrix:
            assert len(row) == size, f"Matrix must be square: expected {size} columns, got {len(row)}"
        self.matrix = matrix
    
    @staticmethod
    def read_from_file(file_path: str) -> 'LatencyMatrix':
        """Read a latency matrix from a file and return as LatencyMatrix instance."""
        matrix = []
        with open(file_path, 'r') as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith('#'):  # Skip empty lines and comments
                    row = [float(x) for x in line.split()]
                    matrix.append(row)
        return LatencyMatrix(matrix)
    
    def get_latency(self, src_idx: int, dst_idx: int) -> float:
        """Get the latency value from source index to destination index."""
        return self.matrix[src_idx][dst_idx]
    
    def size(self) -> int:
        """Get the size of the square matrix."""
        return len(self.matrix)


def address_from_index(index: int) -> str:
    d = index % 254
    c = (index // 254) % 254
    assert c <= 254
    return f"10.0.{c}.{d+1}"


async def oar_job_list_machines(job_id: int) -> list[str]:
    proc = await asyncio.create_subprocess_exec(
        "oarstat",
        "-j",
        str(job_id),
        "-J",
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )

    stdout, stderr = await proc.communicate()

    if proc.returncode != 0:
        raise Exception(f"oarstat failed: {stderr.decode()}")

    # Parse JSON output
    data = json.loads(stdout.decode())
    return data[str(job_id)]["assigned_network_address"]


async def run_script_in_docker(job_id: int, machine: str, script: str, stdin_data: str = None) -> str:
    # Prepare the full script with package installation
    if stdin_data:
        # If stdin_data is provided, create a script that pipes it to the command
        full_script = f"""#!/bin/bash
set -e
apk update >/dev/null 2>&1
apk add iproute2 iproute2-tc >/dev/null 2>&1
cat << 'STDIN_EOF' | {script}
{stdin_data}
STDIN_EOF
"""
    else:
        full_script = f"""#!/bin/bash
set -e
apk update >/dev/null 2>&1
apk add iproute2 iproute2-tc >/dev/null 2>&1
{script}
"""
    
    # Run the script in an Alpine Docker container via SSH
    proc = await asyncio.create_subprocess_exec(
        "oarsh", machine,
        "docker", "run", "--rm", "--privileged", "--net=host",
        "-i", "alpine:latest", "sh",
        env={"OAR_JOB_ID": str(job_id)},
        stdin=asyncio.subprocess.PIPE,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    
    stdout, stderr = await proc.communicate(input=full_script.encode())
    
    if proc.returncode != 0:
        raise Exception(f"Script execution failed on {machine}: {stderr.decode()}")
    
    return stdout.decode()


def machine_interface(name: str) -> str:
    if name not in MACHINE_INTERFACES:
        raise ValueError(f"Unknown machine: {name}")

    interface = MACHINE_INTERFACES[name]
    if interface is None:
        raise ValueError(f"No interface configured for machine: {name}")

    return interface

async def machine_prepare_interface(job_id: int, machine: str):
    interface = machine_interface(machine)
    
    # Get interface information
    get_addr_script = f"ip -j addr show {interface}"
    stdout = await run_script_in_docker(job_id, machine, get_addr_script)
    
    if not stdout.strip():
        print(f"No interface info for {machine}, skipping prepare")
        return
    
    # Parse JSON output
    interface_data = json.loads(stdout)
    
    # Extract addresses that start with '10.'
    commands = []
    for iface in interface_data:
        if "addr_info" in iface:
            for addr in iface["addr_info"]:
                if addr.get("family") == "inet" and addr.get("local", "").startswith("10."):
                    ip = addr["local"]
                    commands.append(f"ip addr del {ip}/32 dev {interface}")
    
    # Remove 10.0.0.0/8 route if it exists
    commands.append(f"ip route del 10.0.0.0/8 dev {interface} 2>/dev/null || true")
    
    if len(commands) == 1:  # Only the route command
        print(f"No 10.x addresses to remove from {machine}, only cleaning up route")
    else:
        print(f"Removing {len(commands)-1} addresses and route from {machine}")
    
    # Execute batch commands
    remove_script = "\n".join(commands)
    await run_script_in_docker(job_id, machine, remove_script)
    
    # Remove existing tc qdiscs separately (ignore errors)
    await run_script_in_docker(job_id, machine, f"tc qdisc del dev {interface} root 2>/dev/null || true")
    
    # Small delay to ensure cleanup is complete
    await asyncio.sleep(0.1)


async def machine_configure_interface(job_id: int, machine: str, address_indices: list[int]):
    interface = machine_interface(machine)
    
    if not address_indices:
        return  # No addresses to add
    
    # Generate IP addresses from indices
    ip_addresses = [address_from_index(idx) for idx in address_indices]
    
    # Prepare ip commands without the 'ip' prefix for batch execution
    commands = []
    for ip in ip_addresses:
        commands.append(f"addr add {ip}/32 dev {interface}")
    
    # Add route for 10.0.0.0/8
    commands.append(f"route add 10.0.0.0/8 dev {interface}")
    
    print(f"Adding {len(ip_addresses)} addresses and route to {machine}")
    
    # Execute batch commands using ip -b -
    commands_data = "\n".join(commands)
    await run_script_in_docker(job_id, machine, "ip -b -", commands_data)


async def machine_configure_latencies(job_id: int, machine: str, address_indices: list[int], latency_matrix: LatencyMatrix):
    interface = machine_interface(machine)
    
    if not address_indices:
        return  # No addresses to configure
    
    # Generate tc commands for latency configuration (without 'tc' prefix)
    commands = []
    
    # Create root qdisc with enough bands for our rules
    max_bands = min(len(address_indices) * latency_matrix.size(), 16)  # prio qdisc supports max 16 bands
    commands.append(f"qdisc add dev {interface} root handle 1: prio bands {max_bands}")
    
    # For each src->dst pair, create a simple netem rule
    filter_counter = 1
    band_counter = 1
    for src_idx in address_indices:
        src_ip = address_from_index(src_idx)
        
        for dst_idx in range(latency_matrix.size()):
            if src_idx != dst_idx and src_idx < latency_matrix.size() and dst_idx < latency_matrix.size():  # Skip self-to-self and out-of-bounds
                dst_ip = address_from_index(dst_idx)
                latency = latency_matrix.get_latency(src_idx, dst_idx)
                
                # Use bands cyclically since prio has limited bands
                band = (band_counter % max_bands) + 1
                
                # Create a unique handle for this rule (must be different from root handle)
                handle = f"{filter_counter + 100}:"
                
                # Add netem qdisc to the appropriate band
                commands.append(f"qdisc add dev {interface} parent 1:{band} handle {handle} netem delay {latency}ms")
                
                # Add filter to match traffic from src_ip to dst_ip
                commands.append(f"filter add dev {interface} protocol ip parent 1: prio {filter_counter} u32 match ip src {src_ip} match ip dst {dst_ip} flowid 1:{band}")
                
                filter_counter += 1
                band_counter += 1
    
    if not commands:
        print(f"No latency configuration needed for {machine}")
        return
    
    print(f"Configuring latencies with {filter_counter-1} rules on {machine}")
    
    # Execute batch tc commands using tc -b -
    tc_commands = "\n".join(commands)
    await run_script_in_docker(job_id, machine, "tc -b -", tc_commands)



async def main():
    parser = argparse.ArgumentParser(description="OAR P2P network setup")
    parser.add_argument("job_id", type=int, help="OAR job ID")
    parser.add_argument("addresses", type=int, help="Number of addresses to allocate")
    parser.add_argument("latency_matrix", type=str, help="Path to latency matrix file")

    args = parser.parse_args()

    # Load latency matrix
    latency_matrix = LatencyMatrix.read_from_file(args.latency_matrix)
    
    machines = await oar_job_list_machines(args.job_id)
    addresses_per_machine = (args.addresses + len(machines) - 1) // len(machines)

    machine_indices = []
    for machine_idx, machine in enumerate(machines):
        indices = []
        for addr_idx in range(addresses_per_machine):
            index = machine_idx * addresses_per_machine + addr_idx
            indices.append(index)
        machine_indices.append(indices)

    print(f"Machines: {machines}")
    print(f"Addresses per machine: {addresses_per_machine}")
    print(f"Machine indices: {machine_indices}")

    # Prepare and configure interfaces for each machine in parallel
    async def prepare_and_configure_machine(machine_idx: int, machine: str):
        print(f"Preparing interface for {machine}...")
        await machine_prepare_interface(args.job_id, machine)
        
        print(f"Configuring interface for {machine}...")
        await machine_configure_interface(args.job_id, machine, machine_indices[machine_idx])
        
        print(f"Configuring latencies for {machine}...")
        await machine_configure_latencies(args.job_id, machine, machine_indices[machine_idx], latency_matrix)
    
    # Run all machines in parallel
    tasks = [
        prepare_and_configure_machine(machine_idx, machine)
        for machine_idx, machine in enumerate(machines)
    ]
    await asyncio.gather(*tasks)


if __name__ == "__main__":
    asyncio.run(main())

