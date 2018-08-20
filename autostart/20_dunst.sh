#!/usr/bin/env bash

systemd-run --remain-after-exit --user --setenv=DISPLAY=${DISPLAY} dunst -config ~/.config/dunstrc &
