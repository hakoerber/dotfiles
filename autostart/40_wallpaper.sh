#!/usr/bin/env bash

wallpaper="$LIBDIR/wallpaper/current"

printf '%s' "setting wallpaper"
systemd-run --property=Restart=always --user --setenv=DISPLAY=${DISPLAY} feh --bg-scale "${wallpaper}"
