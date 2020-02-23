#/usr/bin/env bash
#
# Makes sure the dotfiles directory is in /var/lib/dotfiles and then calls
# install.sh

set -o errexit
set -o nounset

DOTDIR="/var/lib/dotfiles"

_SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

if [[ "$(readlink "${_SCRIPT_DIR}")" != "${DOTDIR}" ]] ; then
    if [[ -e "${DOTDIR}" ]] ; then
        2>&1 printf "${DOTDIR} already exists. This seems unsafe.\n"
        exit 1
    fi
    printf "Moving directory to $DOTDIR ...\n"
    sudo mv --no-target-directory "${_SCRIPT_DIR}" "${DOTDIR}"
    printf "Done\n"
else
    printf "Already working in ${DOTDIR}, nothing to do\n"
fi

cd "${DOTDIR}" && ./install.sh
