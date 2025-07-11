#!/usr/bin/env bash
scripts/generate-schedule.sh | oar-p2p run --output-dir benchmark-logs/ && scripts/benchmark-startup
