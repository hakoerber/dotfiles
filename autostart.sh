#!/usr/bin/env bash

set -o nounset
# set -x

selective=0
if (( $# > 0 )) ; then
    selective=1
    services=("${@}")
fi

do_run() {
    name="$1"
    shift

    run=0
    if (( $selective )) ; then
        for s in "${services[@]}" ; do
            if [[ "$s" == "$name" ]] ; then
                run=1
            fi
        done
    else
        run=1
    fi

    return $(( ! $run ))
}

run_raw() {
    name="$1"
    shift

    do_run "$name" || return

    systemd-run \
        --user \
        --unit "${name}" \
        --no-block \
        --remain-after-exit \
        --setenv=DISPLAY=${DISPLAY} \
        "${@}"
}

run() {
    name="$1"
    shift

    run_raw "$name" --property=Restart=always "${@}"
}

run_oneshot_multiple() {
    name="$1"
    shift

    cmd=()
    # hacky way to start two processes in oneshot mode
    for c in "${@}" ; do
        cmd+=(--property "ExecStart=/usr/bin/env ${c}")
    done
    cmd+=(true)

    run_oneshot "$name" "${cmd[@]}"
}

run_oneshot() {
    name="$1"
    shift

    run_raw "$name" --service-type=oneshot "${@}"
}

schedule() {
    name="$1"; shift
    spec="$1"; shift
    do_run "$name" || return
    systemd-run \
        --user \
        --unit "${name}" \
        --no-block \
        --setenv=DISPLAY=${DISPLAY} \
        --on-calendar="${spec}" \
        "${@}"
}


# redshift settings
redshift_lat_long="49.5:11"
redshift_colortemp="6000:4000"

# wallpaper config
wallpaper="$LIBDIR/wallpaper/current"

# keyboard settings
keyboard_layout=de
keyboard_variant=nodeadkeys
keyboard_repeat_delay=150
keyboard_repeat_speed=50

run_oneshot acpi bash -c 'grep "^${ACPI_LID_NAME}.*enabled" /proc/acpi/wakeup && echo " ${ACPI_LID_NAME}" | sudo tee /proc/acpi/wakeup'

run_oneshot xresources xrdb -merge -I${HOME} ~/.Xresources

run_oneshot_multiple keyboard \
    "setxkbmap -layout $keyboard_layout -variant $keyboard_variant" \
    "xset r rate $keyboard_repeat_delay $keyboard_repeat_speed"

run_oneshot_multiple touchpad \
    "synclient VertEdgeScroll=0" \
    "synclient VertTwoFingerScroll=1" \
    "synclient MaxSpeed=2.2" \
    "synclient AccelFactor=0.08" \
    "synclient TapButton1=1" \
    "synclient CoastingSpeed=0" \
    "synclient PalmDetect=1" \
    "synclient PalmMinWidth=20" \
    "synclient PalmMinZ=180"

run_oneshot pulseaudio start-pulseaudio-x11 --start --daemonize=false --fail=true --log-target=stderr

run gpg-agent gpg-agent --homedir "$HOME/.gnupg" --no-detach --daemon

# a service called dunst already exists and conflicts
run dunst_user dunst -config ~/.config/dunstrc

# disabled due to firefox flicker
# run compton compton --backend glx --vsync opengl --no-dock-shadow --no-dnd-shadow

run_oneshot wallpaper --property=ExecStartPre="/bin/sleep 1" feh --bg-scale "${wallpaper}"

run blueman blueman-applet

run nm-applet nm-applet

run pasystray pasystray

run redshift redshift-gtk -b 1 -l "$redshift_lat_long" -t "$redshift_colortemp"

if [[ "${MACHINE_HAS_KEEPASSX}" ]] ; then
    run keepassx keepassx --keyfile ~/.secret/main.key ~/.secret/main.kdbx
fi

run spotify spotify

if [[ "${MACHINE_HAS_NEXTCLOUD}" ]] ; then
    run nextcloud nextcloud
fi

if [[ "${MACHINE_HAS_RESTIC_BACKUP}" ]] ; then
    [[ -x ~/bin/restic-backup ]] && schedule restic-backup "Mon..Fri 12:00:00" ~/bin/restic-backup
fi


run firefox firefox -P default
