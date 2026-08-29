#!/usr/bin/env bash
#
# Wraps "make" and ensures basic packages are installed:
#
# - make itself
# - python3-virtualenv (needed for ansible)

set -o errexit
set -o nounset

# Make sure to standardize locale, regardless of the machine config
#
# Having a different locale broke "yes | pacman -S" to force-install
# iptables, for example
export LC_ALL="en_US.UTF-8"

DOTDIR="/var/lib/dotfiles"

sudowrap() {
    if (($(id -u) != 0)); then
        sudo "${@}"
    else
        "${@}"
    fi
}

cache_updated=0
install() {
    local package="$1"
    shift

    if [[ "$(lsb_release --short --id)" == "Arch" ]]; then
        if ((!cache_updated)); then
            sudowrap pacman -Sy
            cache_updated=1
        fi
        sudowrap pacman -S --needed --noconfirm "${package}"
    else
        2>&1 printf 'Unsupported distro, exiting\n'
        exit 1
    fi
}

command -v make > /dev/null || install "make"
command -v ansible > /dev/null || install "ansible"

cd "${DOTDIR}" && make
