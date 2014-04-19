#!/bin/bash

# index of the sink. execute pactl list sinks to get a list
SINK=1

getvol() {
    echo $(pactl list sinks | grep "^[[:space:]]*Volume" | head -n $(( $SINK + 1 )) | tail -n 1 | grep -o "[[:digit:]]*%" | head -n 1 | cut -d "%" -f 1)
}

setvol() {
    pactl set-sink-volume $SINK $(( $1 * 65536 / 100 ))
}

ismuted() {
    muted=$(pactl list sinks | grep "^[[:space:]]*Mute" | head -n $(( $SINK + 1 )) | tail -n 1 | cut -d " " -f 2)
    if [[ $muted == "no" ]]; then
        echo 0
    else
        echo 1
    fi
}

mute() {
    pactl set-sink-mute $SINK 1
}

unmute() {
    pactl set-sink-mute $SINK 0
}

mute-toggle() {
    pactl set-sink-mute $SINK toggle
}

status() {
    if [[ $(ismuted) == "1" ]] ; then
        echo "mute"
        return
    fi
    echo "$(getvol)%"
}

usage() {
    echo "Usage:"
    echo
    echo "$0 get-vol"
    echo "$0 set-vol VOL_PERC"
}

case "$1" in
    "get-vol")
        echo $(getvol)
        ;;
    "set-vol")
        if [[ -z "$2" ]] ; then
            usage
        else
            setvol "$2"
        fi
        ;;
    "mute")
        mute
        ;;
    "unmute")
        unmute
        ;;
    "mute-toggle")
        mute-toggle
        ;;
    "is-muted")
        echo $(ismuted)
        ;;
    "status")
        echo $(status)
        ;;
    *)
        echo "wrong usage"
        ;;
esac
