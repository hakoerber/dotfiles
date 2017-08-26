#!/usr/bin/env bash

printf '%s' "starting pasystray" >>"$LOGFILE"
pasystray & &>> $LOGFILE
