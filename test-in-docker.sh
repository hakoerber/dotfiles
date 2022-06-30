#!/usr/bin/env bash

set -o nounset
set -o errexit

tmpdir="$(mktemp -d)"

trap "rm -rf ${tmpdir}" EXIT

git archive --format tar --output "${tmpdir}/dotfiles.tar" HEAD

git submodule foreach 'bash -x -c "
    set -o errexit
    git archive --prefix $path/ HEAD --output '"${tmpdir}/submod.tar"'
    tar -i --concatenate --file='"${tmpdir}/dotfiles.tar"' '"${tmpdir}/submod.tar"'
    rm '"${tmpdir}/submod.tar"'
    "'

gzip -k -f -v "${tmpdir}/dotfiles.tar"

test_ares() {
    if [[ -d "/var/cache/pacman/pkg/" ]] ; then
        dockeropts=(-v "/var/cache/pacman/pkg/:/var/cache/pacman/pkg_host/")
    fi
    docker pull docker.io/library/archlinux:base
    docker run \
        -ti \
        --rm \
        -v ${tmpdir}/dotfiles.tar.gz:/tmp/dotfiles.tar.gz:ro \
        --mount type=tmpfs,destination=/var/cache/pacman/pkg/ \
        "${dockeropts[@]}" \
        --hostname ares \
        docker.io/library/archlinux:base \
        sh -c '
            set -o errexit

            # Uncomment CacheDir and append the host pacman cache as cachedir
            # At worst, the cache directory will be ignored if it does not exist
            # Pacman will always prefer the first cache directory, so newly downloaded
            # packages will stay in the container
            sed -i '"'"'s/^#\?\(CacheDir.*\)/\1\nCacheDir = \/var\/cache\/pacman\/pkg_host\//'"'"' /etc/pacman.conf

            mkdir /var/cache/pacman/pkg_host/
            pacman -Syu --noconfirm linux python3
            cd $(mktemp -d)
            tar xf /tmp/dotfiles.tar.gz -C .
            ANSIBLE_EXTRA_ARGS="-e manage_services=false" ./install.sh
        '
}

test_neptune() {
    docker pull docker.io/library/ubuntu:20.04
    docker run -ti --rm -v ${tmpdir}/dotfiles.tar.gz:/tmp/dotfiles.tar.gz:ro --hostname neptune docker.io/library/ubuntu:20.04 sh -c '
        set -o errexit

        cd $(mktemp -d)
        tar xf /tmp/dotfiles.tar.gz -C .
        ANSIBLE_EXTRA_ARGS="-e manage_services=false" ./install.sh
    '
}

test_mars() {
    docker pull docker.io/library/ubuntu:20.04
    docker run -ti --rm -v ${tmpdir}/dotfiles.tar.gz:/tmp/dotfiles.tar.gz:ro --hostname mars docker.io/library/ubuntu:20.04 sh -c '
        set -o errexit

        cd $(mktemp -d)
        tar xf /tmp/dotfiles.tar.gz -C .
        ANSIBLE_EXTRA_ARGS="-e manage_services=false" ./install.sh
    '
}

case "${1:-all}" in
    ares)
        test_ares
        ;;
    neptune)
        test_neptune
        ;;
    mars)
        test_mars
        ;;
    all)
        test_ares
        test_neptune
        test_mars
        ;;
esac
