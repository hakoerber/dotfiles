# redshift settings
redshift_lat_long="49.5:11"
redshift_colortemp="6000:3300"

log "starting redshift-gtk"
redshift-gtk -b 1 -l "$redshift_lat_long" -t "$redshift_colortemp" & &>> "$LOGFILE"
