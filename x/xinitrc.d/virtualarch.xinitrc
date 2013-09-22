#!/bin/bash

# setup for the dualscreen
# VBOX0: main screen, right, 1920x1080
# VBOX1: second screen, left, 1280x1024


xrandr --newmode 1280x1024 109.00  1280 1368 1496 1712  1024 1027 1034 1063 -hsync +vsync
xrandr --addmode VBOX1 1280x1024
xrandr --output VBOX1 --mode 1280x1024
xrandr --output VBOX0 --mode 1920x1080

xrandr --output VBOX1 --left-of VBOX0
