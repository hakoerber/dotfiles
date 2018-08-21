#!/usr/bin/env bash

printf '%s' "starting pasystray"
systemd-run --property=Restart=always --user --setenv=DISPLAY=${DISPLAY} pasystray
