#!/bin/bash

### From http://www.archlinux.org/index.php/i3

LOGFILE="$HOME/.i3/logs/i3exit.log"
LOGFILE_MAXSIZE=100000

FALLBACK_COLOR=000000

[[ $(stat -c%s "$LOGFILE") -gt $LOGFILE_MAXSIZE ]] && >$LOGFILE

log()
{
    echo "$(date "+%Y-%m-%d %H:%M:%S") $1" >> "$LOGFILE"
}

lock()
{
    resolution=$(xrandr | grep '*' | awk '{ print $1 }')
    log "[I] Resolution found: \"$resolution\""
    lockscreen="$HOME/.i3/data/lockscreen_$resolution.png"
    log "[I] Looking for lockscreen at \"$lockscreen\""
    if [[ -f "$lockscreen" ]] ; then
        log "[I] Lockscreen found, will be used as background image."
        background_options="--image $lockscreen"
    else
        log "[W] Lockscreen not  found, using color #$FALLBACK_COLOR as background."
        background_options="--color $FALLBACK_COLOR"
    fi
    i3lock $background_options
}

log "[I] Received signal \"$1\"."

case "$1" in
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
        lock && systemctl suspend
        ;;
    hibernate)
        log "[I] Hibernating."
        lock && systemctl hibernate
        ;;
    reboot)
        log "[I] Rebooting."
        systemctl reboot
        ;;
    shutdown)
        log "[I] Shutting down."
        systemctl poweroff
        ;;
    *)
        echo "Usage: $0 {lock|logout|suspend|hibernate|reboot|shutdown}"
        log "[E] Signal \"$1\" unknown. Aborting."
        exit 2
esac

log "[I] Done."
exit 0
