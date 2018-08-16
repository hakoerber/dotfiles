#!/usr/bin/env bash

# redshift settings
redshift_lat_long="49.5:11"
redshift_colortemp="6000:3300"

printf '%s' "starting redshift-gtk"
redshift-gtk -b 1 -l "$redshift_lat_long" -t "$redshift_colortemp" &
printf '%s' $! > "$RUNDIR"/redshift.${XDG_SESSION_ID}.pid
