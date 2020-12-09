#!/usr/bin/env bash

set -o nounset
set -o errexit

pacman -Sy --noconfirm git # yes its a partial upgrade, but thats just the live cd

cd /root
git clone --recursive https://code.hkoerber.de/hannes/dotfiles.git

./dotfiles/install_scripts/ares.sh /dev/sda

mv /root/dotfiles /mnt/root/dotfiles
cat << EOF > /mnt/root/.bashrc
set -o errexit
/root/dotfiles/install.sh
rm /root/.bashrc
reboot
EOF

read -p "> Remove installation medium and press enter "
reboot
