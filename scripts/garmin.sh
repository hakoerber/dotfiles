#!/usr/bin/env bash

set -o nounset

gid=$(id -g)
uid=$(id -u)

DEV=/dev/disk/by-label/GARMIN
MOUNTPOINT=/mnt
MOUNTOPTS="uid=${uid},gid=${gid}"

SYNC_FOLDERS=(
    Activity
    Settings
    Courses
    Records
    Totals
    Workouts
)

RSYNCOPTS=(
    --verbose
    --recursive
    --checksum
    --itemize-changes
)

if [[ ! -e "${DEV}" ]]; then
    printf "no garmin device present\n" >&2
    exit 1
fi

mnt=$(findmnt --noheadings --first-only --options "${MOUNTOPTS}" --output TARGET "${DEV}")

if [[ -n "${mnt}" ]]; then
    if [[ "${mnt}" == "${MOUNTPOINT}" ]]; then
        printf '%s already correctly mounted\n' "${DEV}"
    else
        printf '%s already mounted somewhere else, aborting\n' "${DEV}" >&2
        exit 1
    fi
else
    if findmnt --mountpoint "${MOUNTPOINT}" >/dev/null; then
        printf "%s already in use\n" "${MOUNTPOINT}" >&2
        exit 1
    else
        sudo mount -o "${MOUNTOPTS}" "${DEV}" "${MOUNTPOINT}"
    fi
fi

for folder in "${SYNC_FOLDERS[@]}"; do
    rsync "${RSYNCOPTS[@]}" "${MOUNTPOINT}/GARMIN/${folder}/" "${HOME}/sync/garmin/${folder}/"
done
