#!/usr/bin/env bash

# Parameters:
#
# $1: Device

set -o xtrace
set -o nounset
set -o errexit

DEVICE="${1:?}"

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

    ${DEVICE}1 : name=uefi      , size=512M , type=uefi
    ${DEVICE}2 : name=boot      , size=512M , type=linux
    ${DEVICE}3 : name=cryptpart ,             type=linux
EOF

# might take a bit for the new partion table to be updated in-kernel
sleep 1

cryptsetup --batch-mode luksFormat --iter-time 1000 ${DEVICE}3
cryptsetup --batch-mode open ${DEVICE}3 cryptpart

pvcreate /dev/mapper/cryptpart
vgcreate vgbase /dev/mapper/cryptpart

lvcreate -L 16G vgbase -n swap
lvcreate -l 100%FREE vgbase -n root

yes | mkfs.fat -F32 ${DEVICE}1
yes | mkfs.ext4 ${DEVICE}2
yes | mkfs.ext4 /dev/vgbase/swap
yes | mkfs.ext4 /dev/vgbase/root

mount /dev/vgbase/root /mnt

mkdir /mnt/efi
mount ${DEVICE}1 /mnt/efi

mkdir /mnt/boot
mount ${DEVICE}2 /mnt/boot

mkswap /dev/vgbase/swap
swapon /dev/vgbase/swap

pacstrap /mnt base linux linux-firmware networkmanager amd-ucode lvm2 grub efibootmgr

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

printf 'ares\n' > /etc/hostname

cat <<EOF > /etc/hosts
127.0.0.1 localhost
::1       localhost
127.0.1.1 ares
EOF

sed -i 's/^HOOKS=.*$/HOOKS=(base udev autodetect keyboard keymap consolefont modconf block encrypt lvm2 filesystems resume fsck)/' /etc/mkinitcpio.conf

mkinitcpio -P

grub-install --target=x86_64-efi --efi-directory=/efi --bootloader-id=GRUB

sed -i "s/^GRUB_CMDLINE_LINUX=.*$/GRUB_CMDLINE_LINUX=\"cryptdevice=UUID=\$(blkid -s UUID -o value ${DEVICE}3):cryptpart root=UUID=\$(blkid -s UUID -o value /dev/vgbase/root)\"/" /etc/default/grub
sed -i "s/^GRUB_CMDLINE_LINUX_DEFAULT=.*$/GRUB_CMDLINE_LINUX_DEFAULT=\"resume=UUID=\$(blkid -s UUID -o value /dev/vgbase/swap)\"/" /etc/default/grub
sed -i 's/^GRUB_DISABLE_RECOVERY=.*$/GRUB_DISABLE_RECOVERY=/' /etc/default/grub

grub-mkconfig -o /boot/grub/grub.cfg

systemctl enable NetworkManager

passwd
CHROOTSCRIPT

chmod +x /mnt/chroot-script.sh
arch-chroot /mnt /chroot-script.sh
rm -f /mnt/chroot-script.sh
