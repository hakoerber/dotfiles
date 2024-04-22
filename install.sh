#!/usr/bin/env bash
#
# Wraps "make" and ensures basic packages are installed:
#
# - make itself
# - python3-virtualenv (needed for ansible)

set -o errexit
set -o nounset

DOTDIR="/var/lib/dotfiles"
_SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

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

if ! command -v git >/dev/null ; then
    printf 'Git not installed, installing ...\n'
    install "git"
    printf 'Done\n'
fi

if ! command -v python3 >/dev/null ; then
    printf 'Python3 not installed, installing ...\n'
    install "python3"
    printf 'Done\n'
fi

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

[[ -e './.git' ]] && git submodule update --init

if [[ "$(readlink "${_SCRIPT_DIR}")" != "${DOTDIR}" ]] && [[ "${_SCRIPT_DIR}" != "${DOTDIR}" ]] ; then
    if [[ -e "${DOTDIR}" ]] ; then
        2>&1 printf "${DOTDIR} already exists. This seems unsafe.\n"
        exit 1
    fi
    printf "Moving directory to $DOTDIR ...\n"
    sudo=""
    if (( $(id -u ) != 0 )) ; then
        sudo=sudo
    fi
    $sudo mv --no-target-directory "${_SCRIPT_DIR}" "${DOTDIR}"
    printf "Done\n"
else
    printf "Already working in ${DOTDIR}, nothing to do\n"
fi

cd "${DOTDIR}"

cd "${DOTDIR}" && make
