#!/usr/bin/env bash

keyboard_layout=de
keyboard_variant=nodeadkeys
keyboard_repeat_delay=150
keyboard_repeat_speed=50

printf '%s' "setting keyboard layout" >>"$LOGFILE"
setxkbmap -layout "$keyboard_layout" -variant "$keyboard_variant" &

printf '%s' "setting key repeat delay" >>"$LOGFILE"
xset r rate "$keyboard_repeat_delay" "$keyboard_repeat_speed" &
