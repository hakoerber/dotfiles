#!/usr/bin/env bash

printf '%s' "parsing .Xresources" >>"$LOGFILE"
xrdb -merge -I${HOME} ~/.Xresources &>> $LOGFILE
