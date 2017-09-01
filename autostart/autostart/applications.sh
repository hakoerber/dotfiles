#!/bin/bash

log "Starting firefox"
firefox &

log "Starting KeePassX"
keepassx &

log "Starting first terminal"
urxvt -title "scratch" -e "tmux new-session -A -s scratch"
