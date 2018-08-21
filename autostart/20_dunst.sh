#!/usr/bin/env bash

systemd-run --property=Restart=always --user --setenv=DISPLAY=${DISPLAY} dunst -config ~/.config/dunstrc &
