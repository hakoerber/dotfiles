#!/usr/bin/env bash

printf '%s' "starting nextcloud"
systemd-run --user --setenv=DISPLAY=${DISPLAY} nextcloud
