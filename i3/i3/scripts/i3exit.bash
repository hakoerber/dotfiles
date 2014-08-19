#!/bin/bash

### From http://www.archlinux.org/index.php/i3

LOGFILE="$LOGDIR/i3/i3exit.log"
LOGFILE_MAXSIZE=100000

FALLBACK_COLOR=000000

touch "$LOGFILE"
[[ $(stat -c%s "$LOGFILE") -gt $LOGFILE_MAXSIZE ]] && >$LOGFILE

log()
{
    echo "$1"
    echo "[$(date +%FT%T)] $1" >> "$LOGFILE"
}

lock()
{
    resolution=$(xrandr | grep '*' | awk '{ print $1 }' | head -n1)
    log "[I] Resolution found: \"$resolution\""
    lockscreen="$HOME/.i3/data/lockscreen/$resolution.png"
    log "[I] Looking for lockscreen at \"$lockscreen\""
    if [[ -f "$lockscreen" ]] ; then
        log "[I] Lockscreen found, will be used as background image."
        background_options="--image $lockscreen -t"
    else
        log "[W] Lockscreen not found, using color #$FALLBACK_COLOR as background."
        background_options="--color $FALLBACK_COLOR"
    fi
    i3lock $background_options #-p win --image "$HOME/pictures/windows-lockscreen.jpg"
    retval=$?
    [[ -z "$1" ]] && sleep 3 && xset dpms force off
    return $retval
}

if [[ "$1" == '-' ]]; then
    read signal
else
    signal="$1"
fi

log "[I] Received signal \"$signal\"."

case "$signal" in
    lock)
        log "[I] Locking session."
        lock
        ;;
    logout)
        log "[I] Exiting i3."
        i3-msg exit
        ;;
    suspend)
        log "[I] Suspending."
        lock "1" && systemctl suspend
        ;;
    hibernate)
        log "[I] Hibernating."
        #lock &&
        systemctl hibernate
        ;;
    reboot)
        log "[I] Rebooting."
        systemctl reboot
        ;;
    shutdown)
        log "[I] Shutting down."
        systemctl poweroff
        ;;
    screen-off)
        log "[I] Turning screen off."
        xset dpms force off
        ;;
    *)
        echo "Usage: $0 {lock|logout|suspend|hibernate|reboot|shutdown}"
        log "[E] Signal \"$signal\" unknown. Aborting."
        exit 2
esac

log "[I] Done."
exit 0
