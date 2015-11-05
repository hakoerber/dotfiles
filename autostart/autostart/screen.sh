log "execute xautorandr"

xautorandr

log "disable screen blanking"
xset -dpms & &>> $LOGFILE
xset s off & &>> $LOGFILE
