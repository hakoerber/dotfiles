#!/usr/bin/env bash

wallpaper_directory="$LIBDIR/wallpaper/current"
wallchanger_pidfile="$RUNDIR/wallchanger.${XDG_SESSION_ID}.pid"
wallpaper_logfile="$LOGDIR/wallpaper.log"
wallpaper_interval="10800"

printf '%s' "starting $path_wallchanger" >>"$LOGFILE"
{
    wallchanger "$wallpaper_directory" "$wallpaper_interval" &
    echo $! > "$wallchanger_pidfile"
} & &>> $LOGFILE
