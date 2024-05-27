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
    2>&1 printf 'Could not find %, exiting\n' "${os_release_file}"
    exit 1
fi

# shellcheck source=/etc/os-release
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
        2>&1 printf 'Unsupported distro %s, exiting\n' "$NAME"
        exit 1
    fi
}

command -v make    >/dev/null || install "make"
command -v ansible >/dev/null || install "ansible"

cd "${DOTDIR}" && make
