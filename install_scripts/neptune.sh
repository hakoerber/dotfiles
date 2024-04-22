#!/usr/bin/env bash

set -o xtrace
set -o nounset
set -o errexit

DEVICE="/dev/nvme0n1"

if [[ ! -b "${DEVICE}" ]] ; then
    printf '%s does not look like a device' "${DEVICE}"
    exit 1
fi

if [[ ! -d /sys/firmware/efi/efivars ]] ; then
    printf 'efivars does not exist, looks like the system is not booted in EFI mode'
    exit 1
fi

loadkeys de-latin1

timedatectl set-ntp true

sed -e 's/\s*\([^#]*\).*/\1/' << EOF | sfdisk ${DEVICE}
    label: gpt
    device: ${DEVICE}

    ${DEVICE}p1 : name=uefi      , size=512M , type=uefi
    ${DEVICE}p2 : name=boot      , size=512M , type=linux
    ${DEVICE}p3 : name=cryptpart ,             type=linux
EOF

# might take a bit for the new partion table to be updated in-kernel
sleep 1

cryptsetup --batch-mode luksFormat --iter-time 1000 ${DEVICE}p3
cryptsetup --batch-mode open ${DEVICE}p3 cryptpart

pvcreate /dev/mapper/cryptpart
vgcreate vgbase /dev/mapper/cryptpart

lvcreate -L 32G vgbase -n swap
lvcreate -l 100%FREE vgbase -n root

yes | mkfs.fat -F32 ${DEVICE}p1
yes | mkfs.ext4 ${DEVICE}p2
yes | mkfs.ext4 /dev/vgbase/swap
yes | mkfs.ext4 /dev/vgbase/root

mount /dev/vgbase/root /mnt

mkdir /mnt/efi
mount ${DEVICE}p1 /mnt/efi

mkdir /mnt/boot
mount ${DEVICE}p2 /mnt/boot

mkswap /dev/vgbase/swap
swapon /dev/vgbase/swap

pacstrap /mnt base linux-zen linux-firmware networkmanager intel-ucode lvm2 grub efibootmgr

genfstab -U /mnt >> /mnt/etc/fstab

cat << CHROOTSCRIPT > /mnt/chroot-script.sh

set -o xtrace
set -o errexit
set -o nounset

ln -sf /usr/share/zoneinfo/Europe/Berlin /etc/localtime
hwclock --systohc

sed -i 's/^#de_DE.UTF-8 UTF-8/de_DE.UTF-8 UTF-8/' /etc/locale.gen
sed -i 's/^#en_US.UTF-8 UTF-8/en_US.UTF-8 UTF-8/' /etc/locale.gen

locale-gen

printf 'LANG=en_US.UTF-8\n' > /etc/locale.conf

printf 'KEYMAP=de-latin1\nFONT=lat2-16\n' > /etc/vconsole.conf

printf 'neptune\n' > /etc/hostname

cat <<EOF > /etc/hosts
127.0.0.1 localhost
::1       localhost
127.0.1.1 neptune
EOF

sed -i 's/^HOOKS=.*$/HOOKS=(base udev autodetect keyboard keymap consolefont modconf block encrypt lvm2 filesystems resume fsck)/' /etc/mkinitcpio.conf

mkinitcpio -P

grub-install --target=x86_64-efi --efi-directory=/efi --bootloader-id=GRUB

sed -i "s/^GRUB_CMDLINE_LINUX=.*$/GRUB_CMDLINE_LINUX=\"cryptdevice=UUID=\$(blkid -s UUID -o value ${DEVICE}p3):cryptpart root=UUID=\$(blkid -s UUID -o value /dev/vgbase/root)\"/" /etc/default/grub
sed -i "s/^GRUB_CMDLINE_LINUX_DEFAULT=.*$/GRUB_CMDLINE_LINUX_DEFAULT=\"resume=UUID=\$(blkid -s UUID -o value /dev/vgbase/swap)\"/" /etc/default/grub
sed -i 's/^GRUB_DISABLE_RECOVERY=.*$/GRUB_DISABLE_RECOVERY=/' /etc/default/grub

grub-mkconfig -o /boot/grub/grub.cfg

systemctl enable NetworkManager

passwd

# enable root autologin on first boot

mkdir /etc/systemd/system/getty@tty1.service.d/
cat << EOF > /etc/systemd/system/getty@tty1.service.d/autologin.conf
[Service]
ExecStart=
ExecStart=-/sbin/agetty -o '-p -f -- \\u' --noclear --autologin root %I $TERM
EOF
# ExecStartPost=/bin/rm /etc/systemd/system/getty@tty1.service.d/autologin.conf
# ExecStartPost=/bin/rmdir /etc/systemd/system/getty@tty1.service.d/

# Run
cat << EOF > /root/.bash_profile
if /var/lib/dotfiles/install.sh ; then
    rm -f /root/.bash_profile
    reboot
fi
EOF
CHROOTSCRIPT

chmod +x /mnt/chroot-script.sh
arch-chroot /mnt /chroot-script.sh
rm -f /mnt/chroot-script.sh
