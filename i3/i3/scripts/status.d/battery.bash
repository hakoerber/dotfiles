#!/bin/bash

# shitty. this is used for both creating a part of the status conkyrc AND for
# creating nice output if you click on the status bar
# what i done depends on the first paramter

PATH_WARN_1="$HOME/.i3/logs/batwarn1"
PATH_WARN_2="$HOME/.i3/logs/batwarn2"

THRESHOLD1=25
THRESHOLD2=5

acpi_output=$(acpi -b)
percent="$(echo "$acpi_output" | cut -d "," -f 2 | cut -d " " -f 2 | cut -d "%" -f 1)"
if [[ $percent == 100 ]] ; then
    status="Full"
else
    status="$(echo "$acpi_output" | cut -d "," -f 1 | cut -d " " -f 3)"
fi
time="$(echo "$acpi_output" | cut -d "," -f 3 | cut -d " " -f 2)"
shortstatus="$(echo $status | cut -c 1)"

pretty() {
    (
    echo "Status:|$status"
    echo "Charge:|${percent}%"
    if [[ $percent != 100 ]] ; then
        echo "Time left:|$time"
    fi
    ) | column -t --separator="|"
}

charging() {
    [[ "$shortstatus" == "C" ]]
}

discharging() {
    [[ "$shortstatus" == "D" ]]
}

threshold1() {
    [[ $percent -le $THRESHOLD1 ]]
}

threshold2() {
    [[ $percent -le $THRESHOLD2 ]]
}

conky() {
    if discharging ; then
        if threshold2 ; then
            if [[ ! -f "$PATH_WARN_2" ]] ; then
                echo > "$PATH_WARN_2"
                notify-send --icon dialog-warning "Battery below 5%"
            fi
        elif threshold1 ; then
            if [[ ! -f "$PATH_WARN_1" ]] ; then
                echo > "$PATH_WARN_1"
                notify-send --icon dialog-warning "Battery below 25%"
            fi
        fi
    fi

    if charging ; then
        [[ -f "$PATH_WARN_1" ]] && rm "$PATH_WARN_1"
        [[ -f "$PATH_WARN_2" ]] && rm "$PATH_WARN_2"
        #if [[ $percent -gt 25 ]] ; then
        #    [[ -f "$PATH_WARN_1" ]] && rm "$PATH_WARN_1"
        #elif [[ $percent -gt 5 ]] ; then
        #    [[ -f "$PATH_WARN_2" ]] && rm "$PATH_WARN_2"
        #fi

    fi

    if threshold2 ; then
        color="#FF0000" # red
    elif threshold1 ; then
        color="#FFFF00" # yellow
    else
        color="#FFFFFF" # white
    fi



    echo "{ \"full_text\" : \"     "${shortstatus} ${percent}% \" , \"color\" : \"$color\" , \"name\" : \"battery\" },
}

if [[ "$1" == "conky" ]] ; then
    conky
else
    pretty
fi


