#!/usr/bin/env bash

set -o nounset
set -o errexit

sudo bash -c "pacman -Sy --needed --noconfirm archlinux-keyring && pacman -Su"

./configure.sh
