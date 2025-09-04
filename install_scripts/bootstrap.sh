#!/usr/bin/env bash

set -o nounset
set -o errexit

host="${1}" ; shift

pacman -Sy --noconfirm git # yes its a partial upgrade, but thats just the live cd

cd /root
git clone --recursive https://code.hkoerber.de/hannes/dotfiles.git

./dotfiles/install_scripts/"${host}".sh

mv /root/dotfiles /mnt/var/lib/dotfiles

read -rp "> Ready for reboot. Press enter for shutdown, then remove the installation media and boot again "

poweroff
