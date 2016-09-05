#!/usr/bin/env bash

log "parsing .Xresources"
xrdb -merge ~/.Xresources &>> $LOGFILE
