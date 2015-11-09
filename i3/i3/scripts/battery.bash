#!/bin/bash

# shitty. this is used for both creating a part of the status conkyrc AND for
# creating nice output if you click on the status bar
# what i done depends on the first paramter

rundir="$RUNDIR/batwarn"
logfile="$LOGDIR/batwarn.log"

[[ ! -d "$rundir" ]] && mkdir -p "$rundir"

PATH_WARN_1="$rundir/batwarn1"
PATH_WARN_2="$rundir/batwarn2"

THRESHOLD1=25
THRESHOLD2=5

acpi_output=$(acpi -b)
if [[ -n "$acpi_output" ]] ; then
    has_battery=1
    percent="$(echo "$acpi_output" | grep -oP '\d+(?=%)')"
    if [[ $percent == 100 ]] ; then
        status="Full"
    else
        status="$(echo "$acpi_output" | grep -oP '(?<=: )\w+(?=,)' )"
    fi
    shortstatus="$(echo $status | cut -c 1)"
else
    has_battery=0
fi

log() {
    echo [$(date +%FT%T)] "$*" >> $logfile
}

charging() {
    [[ "$shortstatus" == "C" ]]
}

discharging() {
    [[ "$shortstatus" == "D" ]]
}

threshold1() {
    (( $percent <= $THRESHOLD1 ))
}

threshold2() {
    (( $percent <= $THRESHOLD2 ))
}

short() {
    if (( has_battery )) ; then
        if discharging ; then
            if threshold2 ; then
                if [[ ! -f "$PATH_WARN_2" ]] ; then
                    log "battery fell below $THRESHOLD2 percent. issuing warning."
                    echo > "$PATH_WARN_2"
                    notify-send --icon dialog-warning "Battery below ${THRESHOLD2}%" --expire-time 0
                fi
            elif threshold1 ; then
                if [[ ! -f "$PATH_WARN_1" ]] ; then
                    log "battery fell below $THRESHOLD1 percent. issuing warning."
                    echo > "$PATH_WARN_1"
                    notify-send --icon dialog-warning "Battery below ${THRESHOLD1}%" --expire-time 30000
                fi
            fi
        fi

        if charging ; then
            if [[ -f "$PATH_WARN_1" ]] ; then
                log "charging now. resetting warnings."
                rm "$PATH_WARN_1"
            fi
            [[ -f "$PATH_WARN_2" ]] && rm "$PATH_WARN_2"
        fi

        if threshold2 ; then
            color="#FF0000" # red
            urgent=1
        elif threshold1 ; then
            color="#FFFF00" # yellow
            urgent=0
        else
            color="#FFFFFF" # white
            urgent=0
        fi

        echo "${shortstatus}  ${percent}%"
        echo
        echo $color
        (( $urgent )) && exit 33
        exit 0
    else
        echo "no battery"
    fi
}

short
