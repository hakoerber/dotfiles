#!/usr/bin/env bash

set -o xtrace
set -o nounset
set -o errexit

DEVICE="/dev/sda"

if [[ ! -b "${DEVICE}" ]] ; then
    printf '%s does not look like a device\n' "${DEVICE}"
    exit 1
fi

loadkeys de-latin1

timedatectl set-ntp true

sed -e 's/\s*\([^#]*\).*/\1/' << EOF | sfdisk --force ${DEVICE}
    label: gpt
    device: ${DEVICE}

    ${DEVICE}p1 : name=grub,       size=1M   , type=21686148-6449-6E6F-744E-656564454649
    ${DEVICE}p2 : name=boot      , size=512M , type=linux
    ${DEVICE}p3 : name=cryptpart ,             type=linux
EOF

# might take a bit for the new partion table to be updated in-kernel
sleep 1

partprobe "${DEVICE}"

while : ; do
    cryptsetup --batch-mode luksFormat --iter-time 1000 ${DEVICE}3
    cryptsetup --batch-mode open --tries 1 ${DEVICE}3 cryptpart && break
done

pvcreate /dev/mapper/cryptpart
vgcreate vgbase /dev/mapper/cryptpart

lvcreate -L 8G vgbase -n swap
lvcreate -l 100%FREE vgbase -n root

yes | mkfs.ext4 ${DEVICE}2
yes | mkfs.ext4 /dev/vgbase/swap
yes | mkfs.ext4 /dev/vgbase/root

mount /dev/vgbase/root /mnt

mkdir /mnt/boot
mount ${DEVICE}2 /mnt/boot

mkswap /dev/vgbase/swap
swapon /dev/vgbase/swap

pacstrap /mnt base linux-zen linux-firmware networkmanager intel-ucode lvm2 grub

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

printf 'hades\n' > /etc/hostname

cat <<EOF > /etc/hosts
127.0.0.1 localhost
::1       localhost
127.0.1.1 hades
EOF

sed -i 's/^HOOKS=.*$/HOOKS=(base udev autodetect keyboard keymap consolefont modconf block encrypt lvm2 filesystems resume fsck)/' /etc/mkinitcpio.conf

mkinitcpio -P

grub-install --target=i386-pc "${DEVICE}"

sed -i "s/^GRUB_CMDLINE_LINUX=.*$/GRUB_CMDLINE_LINUX=\"cryptdevice=UUID=\$(blkid -s UUID -o value ${DEVICE}3):cryptpart root=UUID=\$(blkid -s UUID -o value /dev/vgbase/root)\"/" /etc/default/grub
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
cat << 'EOF' > /root/.bash_profile
    if [[ "\$(tty)" == "/dev/tty1" ]] ; then
        while ! ping -w 3 -c 3 8.8.8.8 ; do
            nmtui
            sleep 5
        done
        rm -rf /etc/systemd/system/getty@tty1.service.d/
        if /var/lib/dotfiles/install.sh ; then
            rm -f /root/.bash_profile
            reboot
        fi
    fi
EOF
CHROOTSCRIPT

chmod +x /mnt/chroot-script.sh
arch-chroot /mnt /chroot-script.sh
rm -f /mnt/chroot-script.sh
