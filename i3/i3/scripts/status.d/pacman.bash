#!/bin/bash
echo "Total packages: $(pacman -Qq | wc -l)"
echo
if ! pacman -Qu >/dev/null 2>&1; then
    echo "Up to date."
else
    echo "Available updates:"
    echo "$(pacman -Qu | column -t)"
fi
