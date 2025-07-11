#!/bin/sh
date -Iseconds
echo $MESSAGE

# Wait for start signal
while [ ! -f /oar-p2p/start ]; do
    sleep 0.25
done

# Print time when start signal received
date -Iseconds

sleep 2
ping -c 3 -I $ADDRESS $REMOTE
sleep 1
