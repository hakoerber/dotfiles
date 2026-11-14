#!/usr/bin/env bash

set -o nounset
set -o errexit

CONTAINER=."/base"

cargo build --features=testing --manifest-path=./cfg/Cargo.toml --release

sudo rm -rf "${CONTAINER}"
mkdir "${CONTAINER}"
sudo pacstrap -c -K -G "${CONTAINER}" base

sudo systemd-nspawn -D "${CONTAINER}" -- sh -c 'passwd --stdin <<< rootpw'
sudo systemd-nspawn -D "${CONTAINER}" -- sed -i 's/^#\?\(CacheDir.*\)/\1\nCacheDir = \/var\/cache\/pacman-cache-host\//' /etc/pacman.conf

sudo systemd-nspawn --network-veth -b -D "${CONTAINER}" --bind-ro /var/cache/pacman/pkg/:/var/cache/pacman-cache-host --bind-ro ./cfg/target/release/cfg:/usr/local/bin/cfg --bind-ro ./secrets.example.json:/usr/local/share/secrets.json -- sh -c "/usr/local/bin/cfg --hostname dionysus deploy --only-changed --secrets-stdin < /usr/local/share/secrets.json"
