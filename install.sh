#!/usr/bin/env bash
#
# Wraps "make" and ensures basic packages are installed:
#
# - make itself
# - python3-virtualenv (needed for ansible)

set -o errexit
set -o nounset

DOTDIR="/var/lib/dotfiles"

os_release_file=/etc/os-release
if [[ ! -e "${os_release_file}" ]] ; then
    2>&1 printf "Could not find ${os_release_file}, exiting"
    exit 1
fi

source "${os_release_file}"

sudowrap() {
    if (( $(id -u) != 0 )) ; then
        sudo "${@}"
    else
        "${@}"
    fi
}

cache_updated=0
install() {
    local package="$1" ; shift

    if [[ $NAME == "Arch Linux" ]] ; then
        if (( ! cache_updated )) ; then
            sudowrap pacman -Sy
            cache_updated=1
        fi
        sudowrap pacman -S --needed --noconfirm "${package}"
    else
        2>&1 printf "Unsupported distro $NAME, exiting"
        exit 1
    fi
}

if ! command -v make >/dev/null ; then
    printf 'Make not installed, installing ...\n'
    install "make"
    printf 'Done\n'
fi

if ! command -v ansible >/dev/null ; then
    printf 'Ansible not installed, installing ...\n'
    install "ansible"
    printf 'Done\n'
fi

cd "${DOTDIR}" && make
