#!/usr/bin/env bash

printf '%s\n' "execute xautorandr" >>"$LOGFILE"

xautorandr &

printf '%s\n' "start compton" >>"$LOGFILE"
compton --backend glx --vsync opengl &

printf '%s\n' "disable screen blanking" >>"$LOGFILE"
xset -dpms & &>> $LOGFILE
xset s off & &>> $LOGFILE
