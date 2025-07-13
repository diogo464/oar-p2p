# oar-p2p

oar-p2p is a tool to help setup a network, for peer to peer protocol experiments, between one or more machines inside NOVA's cluster.

## prerequisites

### 1. cluster access
cluster access over ssh is required. you can find out more about the cluster here [http://cluster.di.fct.unl.pt](http://cluster.di.fct.unl.pt).

### 2. ssh config
you must be able to access the frontend using pub/priv key authentication and using a single hostname (ex: `ssh dicluster`). the cluster's documentation contains more information on how to set this up at [http://cluster.di.fct.unl.pt/docs/usage/getting_started/](http://cluster.di.fct.unl.pt/docs/usage/getting_started/).

### 3. ssh between machines
once you have access to the frontend you will need to be able to ssh to the various cluster machines using pub/priv key auth (ex: `ssh gengar-1` should work). if you don't already have this setup you can run the following commands from the frontend:
```bash
ssh-keygen -t ed25519 -N "" -f ~/.ssh/id_ed25519
cat ~/.ssh/id_ed25519.pub >> ~/.ssh/authorized_keys
```

### 4. install the tool
to install the tool you have a few options.
+ 1. install using cargo (`cargo install --locked --git https://github.com/diogo464/oar-p2p`)
+ 2. download and extract the binary from one the release assets [https://github.com/diogo464/oar-p2p/releases/latest](https://github.com/diogo464/oar-p2p/releases/latest)
+ 3. clone and compile from source

just make sure the binary ends up somewhere in your `PATH`.

## usage

### 1. setup environment
before setting up a network you need to create a job on the cluster and setup some environment variables. the environment variables are not required since you can pass these values as arguments but it makes it easier.
```bash
export OAR_JOB_ID="<your job id>"
export FRONTEND_HOSTNAME="<cluster's hostname, ex: dicluster>"
```
you can now use a tool like [direnv](https://direnv.net) or just `source` the file with those variables.

### 2. creating the network
to create a network you will need a latency matrix. you can generate a sample using [bonsai](https://codelab.fct.unl.pt/di/computer-systems/bonsai) or using the [web version](https://bonsai.d464.sh).
Here is an example matrix:
```
cat << EOF > latency.txt
0.0 25.5687 78.64806 83.50032 99.91315
25.5687 0.0 63.165894 66.74037 110.71518
78.64806 63.165894 0.0 2.4708898 93.90618
83.50032 66.74037 2.4708898 0.0 84.67561
99.91315 110.71518 93.90618 84.67561 0.0
EOF
```

once you have the latency matrix run:
```bash
# this will create 4 address in total, across the job machines
# it is also possible to specify a number of addresses per machine or per cpu
# 4/cpu will create 4 addressses per cpu on every machine
# 4/machine will create 4 addresses per machine on every machine
oar-p2p net up --addresses 4 --latency-matrix latency.txt
```

to view the created network and the nodes they are on run:
```bash
oar-p2p net show
```

which should output something like
```
gengar-1 10.16.0.1
gengar-1 10.16.0.2
gengar-2 10.17.0.1
gengar-2 10.17.0.2
```

at this point the network is setup, you can check if the latencies are working properly by running a ping
```
~/d/d/oar-p2p (main)> ssh -J cluster gengar-1 ping -I 10.16.0.1 10.17.0.2 -c 3
PING 10.17.0.2 (10.17.0.2) from 10.16.0.1 : 56(84) bytes of data.
64 bytes from 10.17.0.2: icmp_seq=1 ttl=64 time=166 ms
64 bytes from 10.17.0.2: icmp_seq=2 ttl=64 time=166 ms
64 bytes from 10.17.0.2: icmp_seq=3 ttl=64 time=166 ms

--- 10.17.0.2 ping statistics ---
3 packets transmitted, 3 received, 0% packet loss, time 2003ms
rtt min/avg/max/mdev = 166.263/166.300/166.366/0.046 ms
```
which shows the expected latency that is about 2x88ms between address 0 and 3 in the matrix.

### 3. removing the network
this step is optional since the network up command already clears everything before setup, but if you want to remove all the addresses and nft/tc rules just run:
```bash
oar-p2p net down
```

### 4. running containerized experiments
afer having setup the network, how you run the experiments is up to you, but `oar-p2p` has a helper subcommand to automate the process of starting containers, running them and collecting all the logs.

the subcommand is `oar-p2p run` and it requires a "schedule" file to run. a schedule is a json array of objects, where each object describes a container to be executed. here is an example:
```bash
cat << EOF | oar-p2p run --output-dir logs
[
    { 
        "address": "10.16.0.1", 
        "image": "ghcr.io/diogo464/oar-p2p/demo:latest", 
        "env": { "ADDRESS": "10.16.0.1", "REMOTE": "10.17.0.1", "MESSAGE": "I am container 1" }
    },
    { 
        "address": "10.17.0.1", 
        "image": "ghcr.io/diogo464/oar-p2p/demo:latest", 
        "env": { "ADDRESS": "10.17.0.1", "REMOTE": "10.16.0.1", "MESSAGE": "I am container 2" }
    }
]
EOF
```

when the command finishes running the logs should be under the `logs/` directory and contain something like:
```
───────┬────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
       │ File: logs/10.16.0.1.stderr   <EMPTY>
───────┴────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
───────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
       │ File: logs/10.16.0.1.stdout
───────┼────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   1   │ I am container 1
   2   │ PING 10.17.0.1 (10.17.0.1) from 10.16.0.1: 56 data bytes
   3   │ 64 bytes from 10.17.0.1: seq=0 ttl=64 time=50.423 ms
   4   │ 64 bytes from 10.17.0.1: seq=1 ttl=64 time=50.376 ms
   5   │ 64 bytes from 10.17.0.1: seq=2 ttl=64 time=50.356 ms
   6   │
   7   │ --- 10.17.0.1 ping statistics ---
   8   │ 3 packets transmitted, 3 packets received, 0% packet loss
   9   │ round-trip min/avg/max = 50.356/50.385/50.423 ms
───────┴────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
───────┬────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
       │ File: logs/10.17.0.1.stderr   <EMPTY>
───────┴────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
───────┬────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
       │ File: logs/10.17.0.1.stdout
───────┼────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
   1   │ I am container 2
   2   │ PING 10.16.0.1 (10.16.0.1) from 10.17.0.1: 56 data bytes
   3   │ 64 bytes from 10.16.0.1: seq=0 ttl=64 time=50.421 ms
   4   │ 64 bytes from 10.16.0.1: seq=1 ttl=64 time=50.375 ms
   5   │ 64 bytes from 10.16.0.1: seq=2 ttl=64 time=50.337 ms
   6   │
   7   │ --- 10.16.0.1 ping statistics ---
   8   │ 3 packets transmitted, 3 packets received, 0% packet loss
   9   │ round-trip min/avg/max = 50.337/50.377/50.421 ms
───────┴────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
```

#### signals
the run subcommand tries to start all containers at the same time but even then, when running hundreds of containers, some of them will start tens of seconds apart from each other. to help synchronize container start up this subcommand also provides a way to signal containers.
a signal is an empty file located under the `/oar-p2p/` directory that is visible to the container. you can add code inside your container to loop and wait until a certain file exists under this dirctory. for example, starting containers with the following command:
```
oar-p2p run --output-dir logs --signal start:10
```
will make the file `/oar-p2p/start` visibile to all containers 10 seconds after all containers are done starting. if the code inside the containers is made to wait for this file to appear then it is possible for all containers to start up within milliseconds of each other. here is some example java code you might use:
```java
import java.nio.file.Files;
import java.nio.file.Path;

public static void waitForStartFile() {
    Path startFile = Path.of("/oar-p2p/start");
    while (!Files.exists(startFile)) {
        try {
            Thread.sleep(250);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            break;
        }
    }
}
```
