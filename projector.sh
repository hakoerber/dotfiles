#!/usr/bin/env bash

switch_back() {
    screencfg --setup laptop-only
}
trap switch_back EXIT

xrandr --output eDP-1 --off --output DP-1 --auto # --mode 1920x1080


printf 'press ENTER or CTRL+C to switch back'
read -r _
