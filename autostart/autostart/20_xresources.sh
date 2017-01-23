#!/usr/bin/env bash

printf '%s' "parsing .Xresources" >>"$LOGFILE"
xrdb -merge ~/.Xresources &>> $LOGFILE
