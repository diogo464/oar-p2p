#!/usr/bin/env bash
scripts/generate-schedule.sh | oar-p2p run --signal start:10 --output-dir benchmark-logs/ && scripts/benchmark-startup
