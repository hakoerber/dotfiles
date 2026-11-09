#!/usr/bin/env bash

set -o nounset
set -o errexit

CONTAINER=."/base"

cargo build --manifest-path=./cfg/Cargo.toml --release --target=x86_64-unknown-linux-musl
cp -f ./cfg/target/x86_64-unknown-linux-musl/release/cfg ./run

sudo rm -rf "${CONTAINER}"
mkdir "${CONTAINER}"
sudo pacstrap -c -K -G "${CONTAINER}" base

sudo systemd-nspawn -D "${CONTAINER}" -- sh -c 'passwd --stdin <<< root'
sudo systemd-nspawn -D "${CONTAINER}" -- sed -i 's/^#\?\(CacheDir.*\)/\1\nCacheDir = \/var\/cache\/pacman-cache-host\//' /etc/pacman.conf

sudo systemd-nspawn -b -D "${CONTAINER}" --bind /var/cache/pacman/pkg/:/var/cache/pacman-cache-host --bind ./run:/app/run
