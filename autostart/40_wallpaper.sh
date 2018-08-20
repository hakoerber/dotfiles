#!/usr/bin/env bash

wallpaper="$LIBDIR/wallpaper/current"

printf '%s' "setting wallpaper"
systemd-run --remain-after-exit --user --setenv=DISPLAY=${DISPLAY} feh --bg-scale "${wallpaper}"
