#!/usr/bin/env bash

log "starting network tray application"
nm-applet & &>> $LOGFILE
