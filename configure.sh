#!/usr/bin/env bash

set -o nounset
set -o errexit

git submodule update --init ./pkgbuilds

cargo +nightly build --locked --release --bin cfg --manifest-path ./cfg/Cargo.toml

sudo ./cfg/target/release/cfg --hostname "$(hostnamectl hostname)" deploy --only-changed "${@}"
