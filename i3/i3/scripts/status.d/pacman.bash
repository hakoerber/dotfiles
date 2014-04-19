#!/bin/bash

echo "Total packages: $(pacman -Qq | wc -l)"
echo ""
echo "Updates available:"
echo "$(pacman -Qu | column -t)"
