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

os_release_file=/etc/os-release
if [[ ! -e "${os_release_file}" ]] ; then
    2>&1 printf "Could not find ${os_release_file}, exiting"
    exit 1
fi

source /etc/os-release

_SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

sudowrap() {
    if (( $(id -u ) != 0 )) ; then
        sudo "${@}"
    else
        "${@}"
    fi
}

_install() {
    _package="$1" ; shift
    if [[ $NAME == "Fedora" ]] ; then
        sudowrap dnf install --assumeyes "${_package}"
    elif [[ $NAME == "Ubuntu" ]] ; then
        sudowrap apt-get install --assume-yes "${_package}"
    elif [[ $NAME == "Arch Linux" ]] ; then
        sudowrap pacman -S --noconfirm "${_package}"
    else
        2>&1 printf "Unsupported distro $NAME, exiting"
        exit 1
    fi
}


if ! command -v python3 >/dev/null ; then
    printf 'Python3 not installed, installing ...\n'
    _install "python3"
    printf 'Done\n'
fi

if ! command -v make >/dev/null ; then
    printf 'Make not installed, installing ...\n'
    _install "make"
    printf 'Done\n'
fi

# Required for compiling modules in venv.
if ! command -v gcc >/dev/null ; then
    printf 'gcc not installed, installing ...\n'
    _install "gcc"
    printf 'Done\n'
fi

if ! python3 -c 'import venv' 2>/dev/null ; then
    printf 'Python3 venv module not installed, installing ...\n'
    _install python3-venv
    printf 'Done\n'
fi

cd "$_SCRIPT_DIR" && make
