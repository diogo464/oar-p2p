#!/bin/sh
echo $MESSAGE
sleep 2
ping -c 3 -I $ADDRESS $REMOTE
sleep 1
