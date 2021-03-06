#!/usr/bin/env bash

set -o nounset
set -o errexit

pacman -Sy --noconfirm git # yes its a partial upgrade, but thats just the live cd

cd /root
git clone --recursive https://code.hkoerber.de/hannes/dotfiles.git

./dotfiles/install_scripts/ares.sh /dev/sda

mv /root/dotfiles /mnt/root/dotfiles
cat << EOF > /mnt/root/.bash_profile
if /root/dotfiles/install.sh ; then
    rm -f /root/.bash_profile
    reboot
fi
EOF

umount -R /mnt

read -p "> Ready for reboot. Press enter for shutdown, then remove the installation media and boot again "
poweroff
