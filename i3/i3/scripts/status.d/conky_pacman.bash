#!/bin/bash

updates=$(pacman -Quq | wc -l)
if [[ $updates == 0 ]] ; then
    echo "no updates"
else
    echo "$updates updates"
fi
