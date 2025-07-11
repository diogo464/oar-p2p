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
0.0 25.5687 78.64806 83.50032 99.91315
25.5687 0.0 63.165894 66.74037 110.71518
78.64806 63.165894 0.0 2.4708898 93.90618
83.50032 66.74037 2.4708898 0.0 84.67561
99.91315 110.71518 93.90618 84.67561 0.0
```

TODO: update this with addr
once you have the latency matrix run:
```bash
oar-p2p net up --addr-per-cpu 4 --latency-matrix latency.txt
```


