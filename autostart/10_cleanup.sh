#!/usr/bin/env bash

set -o nounset

find "$RUNDIR" -type f -name '*.pid' -delete
