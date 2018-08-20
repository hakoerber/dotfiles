#!/usr/bin/env bash

printf '%s' "starting pasystray"
systemd-run --remain-after-exit --user --setenv=DISPLAY=${DISPLAY} pasystray
