#!/bin/bash

### From http://www.archlinux.org/index.php/i3

LOGFILE="$HOME/.i3/logs/i3exit.log"
LOGFILE_MAXSIZE=100000

[[ $(stat -c%s "$LOGFILE") -gt $LOGFILE_MAXSIZE ]] && >$LOGFILE

log()
{
    echo "$(date "+%Y-%m-%d %H:%M:%S") $1" >> "$LOGFILE"
}

lock()
{
    i3lock --image="$HOME/.i3/data/lockscreen.png"
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
