#!/usr/bin/env bash
set -e
cd $(dirname $0)/..
oar-p2p net up --addresses 4/cpu --latency-matrix ./latency-matrix-5000.txt
scripts/generate-schedule.sh | oar-p2p run --signal start:10 --output-dir benchmark-logs/ && scripts/benchmark-startup
