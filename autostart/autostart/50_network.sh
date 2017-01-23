#!/usr/bin/env bash

printf '%s' "starting network tray application" >>"$LOGFILE"
nm-applet & &>> $LOGFILE
