#!/usr/bin/env bash

set -o nounset
set -o errexit

host="${1}"
shift

cd /root
curl --proto '=https' --tlsv1.2 --fail --output cfg https://de-hkoerber-public.s3.eu-central-1.amazonaws.com/configure

./dotfiles/install_scripts/"${host}".sh

mv /root/dotfiles /mnt/var/lib/dotfiles

read -rp "> Ready for reboot. Press enter for shutdown, then remove the installation media and boot again "

poweroff
