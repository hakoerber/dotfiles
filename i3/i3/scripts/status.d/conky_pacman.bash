#!/bin/bash

updates=$(pacman -Quq | wc -l)
if [[ $updates -eq 0 ]] ; then
    echo "up to date"
elif [[ $updates -eq 1 ]] ; then
    echo "1 update"
else
    echo "$updates updates"
fi
