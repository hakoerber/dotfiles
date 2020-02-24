#!/usr/bin/env bash
#
# Wraps "make" and ensures basic packages are installed:
#
# - make itself
# - python3-virtualenv (needed for ansible)

set -o errexit
set -o nounset

os_release_file=/etc/os-release
if [[ ! -e "${os_release_file}" ]] ; then
    2>&1 printf "Could not find ${os_release_file}, exiting"
    exit 1
fi

source /etc/os-release

_SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

_install() {
    _package="$1" ; shift
    if [[ $NAME == "Fedora" ]] ; then
        sudo dnf install --assumeyes "${_package}"
    elif [[ $NAME == "Ubuntu" ]] ; then
        sudo apt-get install --assume-yes "${_package}"
    else
        2>&1 printf "Unsupported distro $NAME, exiting"
        exit 1
    fi
}

if ! command -v make >/dev/null ; then
    printf 'Make not installed, installing ...\n'
    _install "make"
    printf 'Done\n'
fi

if ! python3 -c 'import venv' 2>/dev/null ; then
    printf 'Python3 venv module not installed, installing ...\n'
    _install python3-venv
    printf 'Done\n'
fi

cd "$_SCRIPT_DIR" && make
