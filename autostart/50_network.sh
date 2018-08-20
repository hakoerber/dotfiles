#!/usr/bin/env bash

printf '%s' "starting network tray application"
systemd-run --remain-after-exit --user --setenv=DISPLAY=${DISPLAY} nm-applet
