#!/usr/bin/env bash

log "starting pasystray"
pasystray & &>> $LOGFILE
