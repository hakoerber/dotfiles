#!/usr/bin/env bash

printf '%s\n' "start compton"
systemd-run --property=Restart=always --user --setenv=DISPLAY=${DISPLAY} compton --backend xrender --vsync opengl &

printf '%s\n' "disable screen blanking"
xset -dpms &
xset s off &

printf '%s\n' "disable wakeup when lid switched"
grep "^${ACPI_LID_NAME}.*enabled" /proc/acpi/wakeup && echo " ${ACPI_LID_NAME}" | sudo tee /proc/acpi/wakeup
