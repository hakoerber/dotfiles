#!/usr/bin/env bash
# From http://i3wm.org/docs/user-contributed/conky-i3bar.html

# the height of the i3bar in pixels. used to align the yad windows.
I3BAR_HEIGHT=24

# the width of the screen in pixels.
X_MAX=1920

# paths to scripts used.
PA_VOLUME_SCRIPT="$HOME/.i3/scripts/pa-volume.bash"
I3BAR_UPDATE="$HOME/.i3/scripts/update-status.bash"
LOGFILE="$LOGDIR/i3/i3log_wrapper.log"
SCRIPTS_SYSINFO="$HOME/.i3/scripts/status.d/sysinfo.bash"
SCRIPTS_WIRELESS="$HOME/.i3/scripts/status.d/wireless.bash"
SCRIPTS_BATTERY="$HOME/.i3/scripts/status.d/battery.bash"
SCRIPTS_PACMAN="$HOME/.i3/scripts/status.d/pacman.bash"
PIDFILE="$LOGDIR/i3/conky.pid"

COLOR_FG="#FFFFFF"
COLOR_BG="#222222"

GTK_THEME="/usr/share/themes/Boje-Greyscale/gtk-2.0/gtkrc"
YAD_PREFIX="GTK2_RC_FILES=$GTK_THEME"

FONT="DejaVu Sans Mono 11"

mkdir -p "$(dirname $LOGFILE)"
echo > "$LOGFILE"

log() {
    echo "[$(date +%FT%T)] $*" >> "$LOGFILE"
}

log "START"

update() {
    bash $I3BAR_UPDATE
}

get_x() {
    x=$1
    width=$2
    pos=$(( $x - ($width / 2) ))
    if [[ $(( $pos + $width )) -gt $X_MAX ]] ; then
        log "window would exceed the right screen border. adjusting"
        pos=$(( $X_MAX - $width ))
    fi
    echo $pos
}

# I'm sorry.
getval() {
    echo $(echo "$1" | grep -Po "\"$2\":.*?," | awk -F ':' '{print $2}' | tr -d '",')
}

path_conkyrc="$1"


# end the header so that i3bar knows we want to use JSON:
echo '{ "version" : 1 , "click_events" : true }'

# Begin the endless array.
echo '['

# We send an empty first array of blocks to make the loop simpler:
echo '[],'

# Now send blocks with information forever:
conky -c "$path_conkyrc" &

echo $! > "$PIDFILE"

while read line ; do
    # This part must not produce any output to stdout as this would be sent
    # to i3bar and crash it due to wrong formatting.

    log "line: $line"
    [[ "$line" == "[" ]] && continue

    name="$(getval "$line" "name")"
    log "name: $name"

    case "$name" in

    "mpd")
        mpc toggle 1>/dev/null 2>%1
        ;;
    "pacman")
        width=500
        height=700
        x=$(get_x $(getval "$line" "x") $width)
        bash "$SCRIPTS_PACMAN" | env $YAD_PREFIX yad \
            --text-info \
            --class=yad-status \
            --button !gtk-apply:0 \
            --button "update"!go-down:1 \
            --button "reload"!gtk-refresh:2 \
            --buttons-layout center \
            --fore "$COLOR_FG" \
            --back "$COLOR_BG" \
            --fontname="$FONT" \
            --geometry ${width}x${height}+${x}+${I3BAR_HEIGHT}
        button=$?
        if [[ $button == 1 ]] ; then
            log "update requested"
            urxvt -e sudo pacman -Syu
        elif [[ $button == 2 ]] ; then
            log "packagelist reload requested"
            urxvt -e sudo pacman -Sy
        fi
        ;;
    "wireless")
        width=500
        height=700
        x=$(get_x $(getval "$line" "x") $width)
        bash "$SCRIPTS_WIRELESS" | env $YAD_PREFIX yad \
            --text-info \
            --class=yad-status \
            --button !gtk-apply:0 \
            --buttons-layout center \
            --fore "$COLOR_FG" \
            --back "$COLOR_BG" \
            --fontname="$FONT" \
            --geometry ${width}x${height}+${x}+${I3BAR_HEIGHT}
        ;;
    "battery")
        width=250
        height=150
        x=$(get_x $(getval "$line" "x") $width)
        bash "$SCRIPTS_BATTERY" | env $YAD_PREFIX yad \
            --text-info \
            --class=yad-status \
            --button !gtk-apply:0 \
            --buttons-layout center \
            --fore "$COLOR_FG" \
            --back "$COLOR_BG" \
            --fontname="$FONT" \
            --geometry ${width}x${height}+${x}+${I3BAR_HEIGHT}
        ;;
    "time")
        width=300
        height=0
        x=$(get_x $(getval "$line" "x") $width)
        env $YAD_PREFIX yad \
            --calendar \
            --class=yad-status \
            --button !gtk-apply:0 \
            --buttons-layout center \
            --geometry ${width}x${height}+${x}+${I3BAR_HEIGHT} >/dev/null
        ;;
    "mpd_status")
        mpc toggle
        ;;
    "sysinfo")
        # i should rather write an extra script
        width=600
        height=800
        x=$(get_x $(getval "$line" "x") $width)
        bash "$SCRIPTS_SYSINFO" | env $YAD_PREFIX yad \
            --text-info \
            --class=yad-status \
            --button !gtk-apply:0 \
            --buttons-layout center \
            --fore "$COLOR_FG" \
            --back "$COLOR_BG" \
            --fontname="$FONT" \
            --geometry ${width}x${height}+${x}+${I3BAR_HEIGHT}
        ;;
    "volume")
        width=300
        height=0
        x=$(get_x $(getval "$line" "x") $width)
        vol=$(bash $PA_VOLUME_SCRIPT get-vol)
        log "got current volume: $vol"
        is_muted=$(bash $PA_VOLUME_SCRIPT is-muted)
        if [[ "$is_muted" == 0 ]] ; then
            button_mute_text="mute"
        else
            button_mute_text="unmute"
        fi
        newvol=$(env $YAD_PREFIX yad \
            --text=Volume: \
            --scale \
            --class=yad-status \
            --button !gtk-apply:0 \
            --button !gtk-cancel:1 \
            --button ${button_mute_text}!:2 \
            --buttons-layout center \
            --value $vol \
            --text-align center \
            --min-value 0 \
            --max-value 100 \
            --step 1 \
            --geometry ${width}x${height}+${x}+${I3BAR_HEIGHT}
        )
        button=$?
        if [[ $button == 2 ]] ; then
            bash $PA_VOLUME_SCRIPT mute-toggle
        elif [[ $button == 0 ]] ; then
            bash $PA_VOLUME_SCRIPT set-vol $newvol
        fi
        update
        ;;
    esac

done
