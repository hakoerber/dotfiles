#!/usr/bin/env bash

set -o nounset
set -o errexit

cpu="$(yaml2json < _machines/"$(hostname --short)".yml | jq --raw-output .cpu)"
gpu="$(yaml2json < _machines/"$(hostname --short)".yml | jq --raw-output .gpu)"

if [[ "${cpu}" ]] ; then
   readarray -d $'\0' -t cpu_packages < <(<drivers.yml yaml2json | jq --raw-output0 ".cpu.${cpu}[]")
fi

if [[ "${gpu}" ]] ; then
   readarray -d $'\0' -t gpu_packages < <(<drivers.yml yaml2json | jq --raw-output0 ".gpu.${gpu}[]")
fi

declare -a aurdeps=()

proctected=(
  base
  java-runtime-common
  jdk17-openjdk
)

for pkgbuild in pkgbuilds/*/PKGBUILD ; do
  source "${pkgbuild}"
  aurdeps+=("${depends[@]%%[<=>]*}" "${makedepends[@]%%[<=>]*}" "${pkgname}")
done

declare -a packages_to_remove=()

readarray -d $'\0' -t packages_to_remove < <(comm --zero-terminated -13 \
  <(cat \
    <(<packages.yml yaml2json | jq --raw-output0 'map(.archlinux) | flatten[]') \
    <(for dep in "${aurdeps[@]}" "${cpu_packages[@]}" "${gpu_packages[@]}" ; do printf '%s\0' "${dep}" ; done) \
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

if (( "${#packages_to_remove[@]}" > 0 )) ; then
    sudo pacman -Rcns "${packages_to_remove[@]}" "${@}" || exit $?
    exit 123
fi

