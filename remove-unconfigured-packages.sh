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
  base # virtual package, would not hurt, but it's weird if its not installed
)

for pkgbuild in pkgbuilds/*/PKGBUILD ; do
  set +o nounset
  # shellcheck disable=SC1090
  source "${pkgbuild}"
  set -o nounset
  aurdeps+=("${depends[@]%%[<=>]*}" "${makedepends[@]%%[<=>]*}" "${checkdepends[@]%%[<=>]*}" "${pkgname}")
done

declare -a packages_to_remove=()

readarray -d $'\0' -t packages_to_remove < <(comm --zero-terminated -13 \
  <(cat \
    <(<_machines/"$(hostname --short)".yml yaml2json | jq --raw-output0 '(.additional_packages // [])[]') \
    <(<packages.yml yaml2json | jq --raw-output0 'map(.archlinux) | flatten[]') \
    <(for dep in "${aurdeps[@]}" "${cpu_packages[@]}" "${gpu_packages[@]}" ; do printf '%s\0' "${dep}" ; done) \
  | while IFS= read -r -d $'\0' package; do
    set -o pipefail
    if resolved_name=$(pacman -Qi "${package}" 2>/dev/null | grep ^Name | grep ^Name | cut -d ':' -f 2 | tr -d ' ') ; then
      if [[ "${resolved_name}" != "${package}" ]] ; then
        if pacman -Qq --explicit "${resolved_name}" >/dev/null ; then
          printf '%s\0' "${resolved_name}"
          continue
        fi
      fi
    fi
    printf '%s\0' "${package}"
    done | sort -zu 
) \
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
    echo "found the following explicitly installed packages that are not configured:"
    for pkg in "${packages_to_remove[@]}" ; do
      echo "${pkg}"
    done
    sudo pacman -Rcns "${packages_to_remove[@]}" "${@}" || exit $?
    exit 123
fi

