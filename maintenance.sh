#!/usr/bin/env bash

set -o nounset
set -o errexit

sudo pacman -Syu

./update-aur-pkgs.sh

ANSIBLE_DISPLAY_OK_HOSTS=false \
ANSIBLE_DISPLAY_SKIPPED_HOSTS=false \
ANSIBLE_EXTRA_ARGS='' \
make
