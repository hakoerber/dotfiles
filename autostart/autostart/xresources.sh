log "parsing .Xresources"
xrdb -merge ~/.Xresources &>> $LOGFILE
