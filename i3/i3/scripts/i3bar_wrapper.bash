#!/usr/bin/env bash
# paths to scripts used.
path_conkyrc="$1"

_PIDFILE="$RUNDIR/i3/conky.pid"


# end the header so that i3bar knows we want to use JSON:
echo '{ "version" : 1 , "click_events" : true }'

# Begin the endless array.
echo '['

# We send an empty first array of blocks to make the loop simpler:
echo '[],'

# Now send blocks with information forever:
conky -c "$path_conkyrc" &

pid=$!

printf '%s' $pid > "$_PIDFILE"

wait $pid
