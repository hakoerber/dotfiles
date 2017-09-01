# wallpaper_directory="$LIBDIR/wallpaper/current"
# wallchanger_pidfile="$RUNDIR/wallchanger.${XDG_SESSION_ID}.pid"
# wallpaper_logfile="$LOGDIR/wallpaper.log"
# wallpaper_interval="10800"

# log "starting wallchanger"
# {
#     wallchanger "$wallpaper_directory" "$wallpaper_interval" &
#     echo $! > "$wallchanger_pidfile"
# } & &>> $LOGFILE
xsetroot -solid '#555555'
