#!/usr/bin/env bash

set -o nounset
set -o errexit

cargo build --locked --release --bin cfg --manifest-path ./cfg/Cargo.toml

sudo ./cfg/target/release/cfg --hostname "$(hostname -s)" --only-changed "${@}"
