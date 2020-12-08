#!/usr/bin/env bash

set -o nounset
set -o errexit

tmpdir="$(mktemp -d)"
tmpdir=/tmp/dotfiles

git archive --format tar --output "${tmpdir}/dotfiles.tar" HEAD

git submodule foreach 'bash -x -c "
    set -o errexit
    git archive --prefix $path/ HEAD --output '"${tmpdir}/submod.tar"'
    tar -i --concatenate --file='"${tmpdir}/dotfiles.tar"' '"${tmpdir}/submod.tar"'
    rm '"${tmpdir}/submod.tar"'
    "'

gzip -k -f -v "${tmpdir}/dotfiles.tar"

exit 1

docker pull docker.io/library/archlinux:base
docker run -ti --rm -v ${tmpdir}/dotfiles.tar.gz:/tmp/dotfiles.tar.gz:ro --hostname ares docker.io/library/archlinux:base sh -c '"
    set -o errexit

    pacman -Syu --noconfirm python3
    cd $(mktemp -d)
    tar xf /tmp/dotfiles.tar.gz -C .
    ANSIBLE_EXTRA_ARGS="-e manage_services=false" ./bootstrap.sh
    read -p "Done, [return] to continue "
'

docker pull docker.io/library/ubuntu:18.04
docker run -ti --rm -v ${tmpdir}/dotfiles.tar.gz:/tmp/dotfiles.tar.gz:ro --hostname tb-hak docker.io/library/ubuntu:18.04 sh -c '
    set -o errexit

    cd $(mktemp -d)
    tar xf /tmp/dotfiles.tar.gz -C .
    ANSIBLE_EXTRA_ARGS="-e manage_services=false" ./bootstrap.sh
'
