path_wallchanger="$HOME/projects/wallchanger/wallchanger"
wallpaper_directory="$LIBDIR/wallpaper/current"
wallchanger_pidfile="$RUNDIR/wallchanger.${XDG_SESSION_ID}.pid"
wallpaper_logfile="$LOGDIR/wallpaper.log"
wallpaper_fallback="$HOME/.i3/data/wallpaper/"
wallpaper_interval="10800"

log "starting $path_wallchanger"
{
    sleep 0
    $path_wallchanger "$wallpaper_directory" "$wallpaper_interval" "$wallpaper_fallback" &
    echo $! > "$wallchanger_pidfile"
} & &>> $LOGFILE
