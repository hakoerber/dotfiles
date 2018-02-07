#!/usr/bin/env bash

wallpaper="$LIBDIR/wallpaper/current"

printf '%s' "setting wallpaper" >>"$LOGFILE"
{
    feh --bg-scale "${wallpaper}"
    sleep 10
    feh --bg-scale "${wallpaper}"
} & &>> $LOGFILE
