#!/usr/bin/env bash

set -o nounset
set -o errexit

git submodule update --init ./pkgbuilds

cargo +nightly build --locked --release --bin cfg --manifest-path ./cfg/Cargo.toml --target x86_64-unknown-linux-musl

aws s3 cp --storage-class STANDARD_IA ./cfg/target/x86_64-unknown-linux-musl/release/cfg s3://de-hkoerber-public/configure
