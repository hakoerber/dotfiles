#!/usr/bin/env bash

for pkg in pkgbuilds/* ; do
    git submodule update --remote "${pkg}"
    if git status --porcelain "${pkg}" | grep -q . ; then
        git add "${pkg}"
        git commit -m "aur: Update $(basename "${pkg}")"
    fi
done
