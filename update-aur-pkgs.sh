#!/usr/bin/env bash

set -o nounset
set -o errexit

for pkg in pkgbuilds/* ; do
    if [[ -n "$(builtin cd "${pkg}" && git rev-parse --show-superproject-working-tree)" ]] ; then
        printf "checking git submodule %s\n" "${pkg}"
        git submodule update --remote "${pkg}"
    else
        printf "checking local package %s\n" "${pkg}"
        (
            builtin cd "${pkg}" || exit 1
            makepkg --nodeps --nobuild --noextract
        )
    fi
    if git status --porcelain "${pkg}" | grep -q . ; then
        git add "${pkg}"
        git commit -m "aur: Update $(basename "${pkg}")"
    fi
done
