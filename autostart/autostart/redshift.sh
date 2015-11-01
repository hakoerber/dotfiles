# redshift settings
redshift_lat_long="49.5:11"
redshift_colortemp="5500:3700"

log "starting redshift-gtk"
redshift-gtk -l "$redshift_lat_long" -t "$redshift_colortemp" & &>> "$LOGFILE"
