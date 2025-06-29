import json
import math
import asyncio
import argparse
import logging

from collections import defaultdict
from dataclasses import dataclass

NFT_TABLE_NAME = "oar-p2p"

MACHINE_INTERFACES = [
    ("alakazam-01", None),
    ("alakazam-02", None),
    ("alakazam-03", None),
    ("alakazam-04", None),
    ("alakazam-05", None),
    ("alakazam-06", None),
    ("alakazam-07", None),
    ("alakazam-08", None),
    ("bulbasaur-1", None),
    ("bulbasaur-2", None),
    ("bulbasaur-3", None),
    ("charmander-1", "bond0"),
    ("charmander-2", "bond0"),
    ("charmander-3", "bond0"),
    ("charmander-4", "bond0"),
    ("charmander-5", "bond0"),
    ("gengar-1", "bond0"),
    ("gengar-2", "bond0"),
    ("gengar-3", "bond0"),
    ("gengar-4", "bond0"),
    ("gengar-5", "bond0"),
    ("kadabra-01", None),
    ("kadabra-02", None),
    ("kadabra-03", None),
    ("kadabra-04", None),
    ("kadabra-05", None),
    ("kadabra-06", None),
    ("kadabra-07", None),
    ("kadabra-08", None),
    ("lugia-1", None),
    ("lugia-2", None),
    ("lugia-3", None),
    ("lugia-4", None),
    ("lugia-5", None),
    ("magikarp-1", None),
    ("moltres-01", None),
    ("moltres-02", None),
    ("moltres-03", None),
    ("moltres-04", None),
    ("moltres-05", None),
    ("moltres-06", None),
    ("moltres-07", None),
    ("moltres-08", None),
    ("moltres-09", None),
    ("moltres-10", None),
    ("oddish-1", None),
    ("psyduck-1", None),
    ("psyduck-2", None),
    ("psyduck-3", None),
    ("shelder-1", None),
    ("squirtle-1", None),
    ("squirtle-2", None),
    ("squirtle-3", None),
    ("squirtle-4", None),
    ("staryu-1", None),
    ("sudowoodo-1", None),
    ("vulpix-1", None),
]


class LatencyMatrix:
    def __init__(self, matrix: list[list[float]]):
        # Assert matrix is not None
        assert matrix is not None, "Matrix cannot be None"
        # Assert matrix is square
        size = len(matrix)
        for row in matrix:
            assert (
                len(row) == size
            ), f"Matrix must be square: expected {size} columns, got {len(row)}"
        self.matrix = matrix

    @staticmethod
    def read_from_file(file_path: str) -> "LatencyMatrix":
        """Read a latency matrix from a file and return as LatencyMatrix instance."""
        matrix = []
        with open(file_path, "r") as f:
            for line in f:
                line = line.strip()
                if line and not line.startswith("#"):  # Skip empty lines and comments
                    row = [float(x) for x in line.split()]
                    matrix.append(row)
        return LatencyMatrix(matrix)

    def get_latency(self, src_idx: int, dst_idx: int) -> float:
        """Get the latency value from source index to destination index."""
        return self.matrix[src_idx][dst_idx]

    def size(self) -> int:
        """Get the size of the square matrix."""
        return len(self.matrix)


@dataclass
class MachineConfiguration:
    machine: str
    addresses: list[str]
    nft_script: str
    tc_commands: list[str]
    ip_commands: list[str]


def machine_get_interface(machine: str) -> str:
    for name, interface in MACHINE_INTERFACES:
        if name == machine:
            assert interface is not None, f"machine interface not configured: {machine}"
            return interface
    raise ValueError(f"Unknown machine: {machine}")


def machine_get_index(machine: str) -> int:
    for i, (name, _) in enumerate(MACHINE_INTERFACES):
        if name == machine:
            return i
    raise ValueError(f"Unknown machine: {machine}")


def machine_generate_configurations(
    machines: list[str], num_addresses_per_machine: int, matrix: LatencyMatrix
) -> list[MachineConfiguration]:
    configurations = []

    machine_addr_idxs = defaultdict(list)
    addr_idx_to_machine_idx = defaultdict(int)
    for machine_idx in range(len(machines)):
        for local_addr_idx in range(num_addresses_per_machine):
            addr_idx = machine_idx * num_addresses_per_machine + local_addr_idx
            machine_addr_idxs[machines[machine_idx]].append(addr_idx)
            addr_idx_to_machine_idx[addr_idx] = machine_get_index(machines[machine_idx])

    for machine in machines:
        machine_ips = []
        machine_index = machine_get_index(machine)
        interface = machine_get_interface(machine)
        addr_idxs = machine_addr_idxs[machine]
        ip_commands = []
        tc_commands = []

        ip_commands.append(f"route add 10.0.0.0/8 dev {interface}")
        for addr_idx in addr_idxs:
            addr = address_from_index(
                machine_index, addr_idx % num_addresses_per_machine
            )
            machine_ips.append(addr)
            ip_commands.append(f"addr add {addr}/32 dev {interface}")

        latencies_set = set()
        latencies_buckets = defaultdict(list)
        for addr_idx in addr_idxs:
            for i in range(num_addresses_per_machine):
                if addr_idx == i:
                    continue
                latency = matrix.get_latency(addr_idx, i)
                latency_rounded = math.ceil(latency) // 1
                latencies_set.add(latency_rounded)
                latencies_buckets[latency_rounded].append((addr_idx, i))

        latencies = list(sorted(latencies_set))

        tc_commands.append(f"qdisc add dev {interface} root handle 1: htb default 9999")
        tc_commands.append(
            f"class add dev {interface} parent 1: classid 1:9999 htb rate 10gbit"
        )
        for idx, latency in enumerate(latencies):
            # tc class for latency at idx X is X + 1
            tc_commands.append(
                f"class add dev {interface} parent 1: classid 1:{idx+1} htb rate 10gbit"
            )
            tc_commands.append(
                f"qdisc add dev {interface} parent 1:{idx+1} handle {idx+2}: netem delay {latency}ms"
            )
            # mark for latency at idx X is X + 1
            tc_commands.append(
                f"filter add dev {interface} parent 1:0 prio 1 handle {idx+1} fw flowid 1:{idx+1}"
            )

        nft_script = ""
        nft_script += "table ip oar-p2p {" + "\n"
        for latency_idx, latency in enumerate(latencies):
            if len(latencies_buckets[latency]) == 0:
                continue
            nft_script += f"  set mark_{latency_idx}_pairs {{\n"
            nft_script += f"    type ipv4_addr . ipv4_addr\n"
            nft_script += f"    flags interval\n"
            nft_script += f"    elements = {{\n"
            for src_idx, dst_idx in latencies_buckets[latency]:
                assert src_idx != dst_idx
                src_addr = address_from_index(
                    addr_idx_to_machine_idx[src_idx],
                    src_idx % num_addresses_per_machine,
                )
                dst_addr = address_from_index(
                    addr_idx_to_machine_idx[dst_idx],
                    dst_idx % num_addresses_per_machine,
                )
                nft_script += f"      {src_addr} . {dst_addr},\n"
            nft_script += f"    }}\n"
            nft_script += f"  }}\n\n"

        nft_script += "    chain postrouting {\n"
        nft_script += "        type filter hook postrouting priority mangle - 1\n"
        nft_script += "        policy accept\n"
        nft_script += "\n"
        for latency_idx in range(len(latencies)):
            nft_script += f"        ip saddr . ip daddr @mark_{latency_idx}_pairs meta mark set {latency_idx+1}\n"
        nft_script += "    }" + "\n"
        nft_script += "}" + "\n"

        configurations.append(
            MachineConfiguration(
                machine=machine,
                addresses=machine_ips,
                nft_script=nft_script,
                tc_commands=tc_commands,
                ip_commands=ip_commands,
            )
        )

    return configurations


async def machine_apply_configuration(job_id: int, config: MachineConfiguration):
    """Apply a machine configuration by executing IP commands, TC commands, and NFT script."""
    machine = config.machine

    logging.info(f"Applying configuration to {machine}...")

    # Prepare tasks for parallel execution
    tasks = []

    # IP commands task
    if config.ip_commands:
        ip_batch = "\n".join(config.ip_commands)
        logging.info(f"Executing {len(config.ip_commands)} IP commands on {machine}")
        tasks.append(run_script_in_docker(job_id, machine, "ip -b -", ip_batch))

    # TC commands task
    if config.tc_commands:
        tc_batch = "\n".join(config.tc_commands)
        logging.info(f"Executing {len(config.tc_commands)} TC commands on {machine}")
        tasks.append(run_script_in_docker(job_id, machine, "tc -b -", tc_batch))

    # NFT script task
    if config.nft_script:
        logging.info(f"Applying NFT script on {machine}")
        tasks.append(
            run_script_in_docker(job_id, machine, "nft -f -", config.nft_script)
        )

    # Execute all tasks in parallel
    if tasks:
        try:
            await asyncio.gather(*tasks)
        except Exception as e:
            logging.error(f"ERROR applying configuration to {machine}: {e}")
            raise


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


async def run_script_in_docker(
    job_id: int, machine: str, script: str, stdin_data: str | None = None
) -> str:
    # Prepare the script (no package installation needed with custom image)
    if stdin_data:
        # If stdin_data is provided, create a script that pipes it to the command
        full_script = f"""#!/bin/bash
set -e
cat << 'STDIN_EOF' | {script}
{stdin_data}
STDIN_EOF
"""
    else:
        full_script = f"""#!/bin/bash
set -e
{script}
"""

    # Run the script in our custom networking Docker container via SSH
    proc = await asyncio.create_subprocess_exec(
        "oarsh",
        machine,
        "docker",
        "run",
        "--rm",
        "--privileged",
        "--pull=always",
        "--net=host",
        "-i",
        "ghcr.io/diogo464/oar-p2p-networking:latest",
        env={"OAR_JOB_ID": str(job_id)},
        stdin=asyncio.subprocess.PIPE,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )

    stdout, stderr = await proc.communicate(input=full_script.encode())

    if proc.returncode != 0:
        cmd_args = [
            "oarsh",
            machine,
            "docker",
            "run",
            "--rm",
            "--privileged",
            "--net=host",
            "-i",
            "ghcr.io/diogo464/oar-p2p-networking:latest",
        ]
        raise Exception(
            f"Script execution failed on {machine}\nCommand: {' '.join(cmd_args)}\nScript: {script}\nStderr: {stderr.decode()}"
        )

    return stdout.decode()


def machine_interface(name: str) -> str:
    for machine, interface in MACHINE_INTERFACES:
        if machine == name:
            if interface is None:
                raise ValueError(f"No interface configured for machine: {name}")
            return interface
    raise ValueError(f"Unknown machine: {name}")


async def machine_cleanup_interface(job_id: int, machine: str):
    interface = machine_interface(machine)

    # Get interface information
    get_addr_script = f"ip -j addr show {interface}"
    stdout = await run_script_in_docker(job_id, machine, get_addr_script)

    if not stdout.strip():
        logging.info(f"No interface info for {machine}, skipping cleanup")
        return

    # Parse JSON output
    interface_data = json.loads(stdout)

    # Extract addresses that start with '10.'
    commands = []
    for iface in interface_data:
        if "addr_info" in iface:
            for addr in iface["addr_info"]:
                if addr.get("family") == "inet" and addr.get("local", "").startswith(
                    "10."
                ):
                    ip = addr["local"]
                    commands.append(f"ip addr del {ip}/32 dev {interface}")

    # Remove 10.0.0.0/8 route if it exists
    commands.append(f"ip route del 10.0.0.0/8 dev {interface} 2>/dev/null || true")

    if len(commands) == 1:  # Only the route command
        logging.info(f"No 10.x addresses to remove from {machine}, only cleaning up route")
    else:
        logging.info(f"Removing {len(commands)-1} addresses and route from {machine}")

    # Execute batch commands and clean TC state and NFT table in parallel
    remove_script = "\n".join(commands)
    tasks = [
        run_script_in_docker(job_id, machine, remove_script),
        run_script_in_docker(
            job_id, machine, f"tc qdisc del dev {interface} root 2>/dev/null || true"
        ),
        run_script_in_docker(
            job_id, machine, f"tc qdisc del dev {interface} ingress 2>/dev/null || true"
        ),
        run_script_in_docker(
            job_id, machine, f"nft delete table {NFT_TABLE_NAME} 2>/dev/null || true"
        ),
    ]
    await asyncio.gather(*tasks)

    # Small delay to ensure cleanup is complete
    await asyncio.sleep(0.2)


async def setup_command(job_id: int, addresses: int, latency_matrix_path: str):
    # Load latency matrix
    latency_matrix = LatencyMatrix.read_from_file(latency_matrix_path)

    # Get machines from job
    machines = await oar_job_list_machines(job_id)

    logging.info(f"Machines: {machines}")
    logging.info(f"Total addresses: {addresses}")

    # Generate configurations for all machines
    configurations = machine_generate_configurations(
        machines, addresses, latency_matrix
    )

    # Apply configurations to each machine in parallel
    async def setup_machine(config: MachineConfiguration):
        if config.machine == "charmander-2":
            return
        logging.info(f"Setting up {config.machine}...")

        # First cleanup the interface
        await machine_cleanup_interface(job_id, config.machine)

        # Then apply the new configuration
        await machine_apply_configuration(job_id, config)

    # Run all machines in parallel
    tasks = [setup_machine(config) for config in configurations]
    await asyncio.gather(*tasks)
    
    # Print machine IP pairs to stdout
    for config in configurations:
        for ip in config.addresses:
            print(f"{config.machine} {ip}")


async def clean_command(job_id: int):
    machines = await oar_job_list_machines(job_id)

    logging.info(f"Cleaning up {len(machines)} machines...")

    # Clean up all machines in parallel, but don't fail fast
    tasks = [machine_cleanup_interface(job_id, machine) for machine in machines]
    results = await asyncio.gather(*tasks, return_exceptions=True)

    # Check for failures after all tasks complete
    failures = []
    for machine, result in zip(machines, results):
        if isinstance(result, Exception):
            failures.append((machine, result))
            logging.error(f"ERROR: Cleanup failed on {machine}: {result}")
        else:
            logging.info(f"Cleanup completed successfully on {machine}")

    if failures:
        failed_machines = [machine for machine, _ in failures]
        raise Exception(
            f"Cleanup failed on {len(failures)} machines: {', '.join(failed_machines)}"
        )

    logging.info("Cleanup completed successfully on all machines")


async def configurations_command(job_id: int, addresses: int, latency_matrix_path: str):
    # Load latency matrix
    latency_matrix = LatencyMatrix.read_from_file(latency_matrix_path)

    # Get machines from job
    machines = await oar_job_list_machines(job_id)

    # Generate configurations
    configurations = machine_generate_configurations(
        machines, addresses, latency_matrix
    )

    # Print configurations with markers between each machine
    for i, config in enumerate(configurations):
        if i > 0:
            print("\n" + "=" * 80 + "\n")

        print(f"Machine: {config.machine}")
        print("-" * 40)

        print("\nAddresses:")
        for addr in config.addresses:
            print(addr)

        print("NFT Script:")
        print(config.nft_script)

        print("\nTC Commands:")
        for cmd in config.tc_commands:
            print(f"tc {cmd}")

        print("\nIP Commands:")
        for cmd in config.ip_commands:
            print(cmd)


async def main():
    # Configure logging to write to stderr
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(levelname)s - %(message)s',
        handlers=[logging.StreamHandler()]
    )
    
    parser = argparse.ArgumentParser(description="OAR P2P network management")
    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # Setup command
    setup_parser = subparsers.add_parser(
        "up", help="Setup network interfaces and latencies"
    )
    setup_parser.add_argument("job_id", type=int, help="OAR job ID")
    setup_parser.add_argument(
        "addresses", type=int, help="Number of addresses to allocate"
    )
    setup_parser.add_argument(
        "latency_matrix", type=str, help="Path to latency matrix file"
    )

    # Clean command
    clean_parser = subparsers.add_parser("down", help="Clean up network interfaces")
    clean_parser.add_argument("job_id", type=int, help="OAR job ID")

    # Configurations command
    config_parser = subparsers.add_parser(
        "configurations", help="Generate and print machine configurations"
    )
    config_parser.add_argument("job_id", type=int, help="OAR job ID")
    config_parser.add_argument(
        "addresses", type=int, help="Number of addresses to allocate per machine"
    )
    config_parser.add_argument(
        "latency_matrix", type=str, help="Path to latency matrix file"
    )

    args = parser.parse_args()

    if args.command == "up":
        await setup_command(args.job_id, args.addresses, args.latency_matrix)
    elif args.command == "down":
        await clean_command(args.job_id)
    elif args.command == "configurations":
        await configurations_command(args.job_id, args.addresses, args.latency_matrix)
    else:
        parser.print_help()


def address_from_index(machine_index: int, addr_index: int) -> str:
    d = addr_index % 254
    c = (addr_index // 254) % 254
    assert c <= 254
    return f"10.{machine_index}.{c}.{d+1}"


if __name__ == "__main__":
    asyncio.run(main())

