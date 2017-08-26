#!/usr/bin/env bash

printf '%s\n' "execute xautorandr" >>"$LOGFILE"

xautorandr

printf '%s\n' "disable screen blanking" >>"$LOGFILE"
xset -dpms & &>> $LOGFILE
xset s off & &>> $LOGFILE
