#!/usr/bin/env bash

line="$1

"

log(){
    echo $* >> ~/.i3/output_i3bar.log.2
}

# I'm sorry.
getval(){
    echo $(echo "$1" | grep -Po "\"$2\":.*?," | awk -F ':' '{print $2}' | tr -d '",')
}

log $line

name="$(getval "$line" "name")"
log $name

case "$name" in
"time")
    log "starting dzen with cal"
    (echo "$(cal -1m)" ; sleep 15s) | dzen2 -w 100 -l 10 -x 1700 -y 20
    ;;
esac

