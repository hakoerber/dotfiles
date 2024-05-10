#!/usr/bin/env bash

declare -a aurdeps=()

proctected=(
  intel-ucode
  amd-ucode
  base
  java-runtime-common
  jdk17-openjdk
  lib32-vulkan-radeon
  vulkan-radeon
  libva-mesa-driver
  mesa-vdpau
  lib32-mesa-vdpau
  vulkan-mesa-layers
  lib32-libva-mesa-driver
)

for pkgbuild in pkgbuilds/*/PKGBUILD ; do
  source "${pkgbuild}"
  aurdeps+=("${depends[@]%%[<=>]*}" "${makedepends[@]%%[<=>]*}" "${pkgname}")
done

packages_to_remove=()

readarray -d $'\0' -t packages_to_remove < <(comm --zero-terminated -13 \
  <(cat \
    <(<packages.yml yaml2json | jq --raw-output0 'map(.archlinux) | flatten[]') \
    <(for dep in "${aurdeps[@]}" ; do printf '%s\0' "${dep}" ; done) \
  | sort -zu) \
  <(pacman -Qq --explicit | xargs -I "{}" printf '%s\0' "{}" | sort -zu) \
| while IFS= read -r -d $'\0' package; do
    skip=0
    for protected in "${proctected[@]}" ; do
      if [[ "${package}" == "${protected}" ]] ; then 
        skip=1
      fi
    done
    if (( skip )) ; then
      continue
    fi
    printf '%s\0' "${package}"
  done) 

if (( "${#packages_to_remove}" > 0 )) ; then
    sudo pacman -Rcns "${packages_to_remove[@]}" "${@}" || exit $?
    exit 123
fi

