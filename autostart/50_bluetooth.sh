#!/usr/bin/env bash

printf '%s' "starting network tray application"
systemd-run --property=Restart=always --user --setenv=DISPLAY=${DISPLAY} blueman-applet
