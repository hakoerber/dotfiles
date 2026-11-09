#!/usr/bin/env bash

set -o nounset
set -o errexit

git submodule update --init ./pkgbuilds

cargo +nightly build --locked --release --bin cfg --manifest-path ./cfg/Cargo.toml

aws s3 cp --storage-class STANDARD_IA ./cfg/target/release/cfg s3://de-hkoerber-public/configure
