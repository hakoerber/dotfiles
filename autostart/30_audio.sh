#!/usr/bin/env bash

printf '%s' "starting pulseaudio" >>"$LOGFILE"
pulseaudio --start
