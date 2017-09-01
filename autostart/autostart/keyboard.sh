#!/bin/bash
keyboard_layout=de
keyboard_variant=nodeadkeys
keyboard_repeat_delay=150
keyboard_repeat_speed=50

log "setting keyboard layout"
setxkbmap -layout "$keyboard_layout" -variant "$keyboard_variant" & &>> $LOGFILE

log "setting key repeat delay"
xset r rate "$keyboard_repeat_delay" "$keyboard_repeat_speed" & &>> $LOGFILE
