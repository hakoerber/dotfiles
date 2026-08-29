#!/usr/bin/env bash

set -o nounset
set -o errexit

sudo bash -c "pacman -Sy --needed --noconfirm archlinux-keyring && pacman -Su"

ANSIBLE_DISPLAY_OK_HOSTS=false \
    ANSIBLE_DISPLAY_SKIPPED_HOSTS=false \
    ANSIBLE_EXTRA_ARGS='' \
    make
