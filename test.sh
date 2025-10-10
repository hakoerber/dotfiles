#!/usr/bin/env bash

set -o nounset
set -o errexit
set -o pipefail

tmpdir="$(mktemp -d --tmpdir=/var/tmp)"

trap cleanup EXIT

ISO_MIRROR="https://ftp.fau.de/archlinux/iso/latest/"
ISO_MIRROR="https://ftp.acc.umu.se/mirror/archlinux/iso/latest/"

iso_dir="${XDG_DATA_HOME}/arch-iso/"
iso_path="${iso_dir}/archlinux-x86_64.iso"

cleanup() {
    rm -rf "${tmpdir}"
    pids=()
    jobs -p | while IFS="" read -r line; do pids+=("$line"); done
    if (( "${#pids[@]}" > 0)) ; then
        kill "${pids[@]}"
    fi
}

download_iso() {
    mkdir -p "${iso_dir}"
    (
        cd "${iso_dir}"
        wget \
            --timestamping \
            --no-hsts \
            "${ISO_MIRROR}sha256sums.txt"

        if [[ ! -e "${iso_path}" ]] || ! sha256sum --ignore-missing --check ./sha256sums.txt; then
            wget \
                --no-hsts \
                --output-document "${iso_path}" \
                "${ISO_MIRROR}archlinux-x86_64.iso"
        fi
    )
}

disk="${tmpdir}/disk.qcow2"

mon_sock="${tmpdir}/mon.sock"

sshopts=(
    -o StrictHostKeyChecking=no
    -o UserKnownHostsFile=/dev/null
    -o PreferredAuthentications=publickey
    -o ConnectTimeout=1s
    -i "${tmpdir}/ssh.key"
    -l root
    -p 60022
    127.0.0.1
)

wait_for_ssh() {
    echo "waiting for ssh"
    set +o errexit
    maxtries=60
    tries=0
    while ! ssh -q "${sshopts[@]}" true; do
        ((tries++))
        if ((tries > maxtries)); then
            echo "ssh did not become available"
            exit 3
        fi
        sleep 1
    done
    echo "ssh available"
    set -o errexit
}

qemuopts=(
    "-m" "size=8G"
    "-drive" "file=${disk},format=qcow2,if=none,id=root"

    "-accel" "kvm"

    "-machine" "q35,smm=on,acpi=on"
    "-smp" "cpus=8,sockets=1,cores=8,threads=1"
    "-cpu" "host"

    "-netdev" "user,id=net0,hostfwd=tcp::60022-:22"
    "-device" "virtio-net-pci,netdev=net0"

    "-nodefaults"

    "-vga" "virtio"
    "-display" "spice-app"
)

send_mon() {
    local socket="${1}"
    patterns=(
        -e 's/ /spc/'
        -e 's/\./dot/'
        -e 's/,/comma/' -e 's/-/slash/'
        -e 's/\//shift-7/'
        -e 's/\([A-Z]\)/shift-\L\1/'
        -e 's/=/shift-0/'
        -e 's/"/shift-2/'
        -e "s/'/shift-0x2b/"
        # ^ is a dead key, we would have to send a space to be precise. but it's
        # going to work out as long as the following char does not combine
        -e 's/\^/0x29/'
        -e 's/#/0x2b/'
        -e 's/\?/shift-0x0c/'
        -e 's/\\/alt_r-0x0c/' # altgr is alt_r
        -e 's/\*/shift-0x1b/'
        -e 's/(/shift-0x09/'
        -e 's/)/shift-0x0a/'
        -e 's/^/sendkey /'
    )

    cat \
        <(fold -w 1 |
            sed "${patterns[@]}") \
        <(echo "sendkey ret") |
        nc -N -U "${socket}"

    echo "sendkey ret" | nc -N -U "${socket}"
}

install_from_iso() {
    local hostname="${1}"
    shift
    local hostqemuopts=("$@")
    rm -rf "${tmpdir:?}"/*

    ssh-keygen -f "${tmpdir}"/ssh.key -N '' -t ed25519 -C 'archiso-tmp'

    cloud-localds "${tmpdir}/userdata.img" <(
        cat <<EOF
    #cloud-config
    users:
      - name: root
        ssh_authorized_keys:
          - $(cat "${tmpdir}"/ssh.key.pub)

EOF
    )

    cp /usr/share/edk2/x64/OVMF_VARS.4m.fd "${tmpdir}/efivars.fd"
    mkisofs \
        -uid 0 \
        -gid 0 \
        -J \
        -R \
        -T \
        -V REPO \
        -o "${tmpdir}/repo.iso" \
        .

    qemu-img create \
        -f qcow2 \
        "${disk}" \
        1000G

    opts=(
        "-cdrom" "${iso_path}"
        "-boot" "order=d"

        "-drive" "file=${tmpdir}/repo.iso,format=raw,if=virtio,media=cdrom"
        "-drive" "file=${tmpdir}/userdata.img,format=raw,if=virtio,media=cdrom"

        "-fsdev" "local,id=pacman-cache,path=share,path=/var/cache/pacman/pkg/,readonly=on,security_model=none"
        "-device" "virtio-9p-pci,fsdev=pacman-cache,mount_tag=pacman-cache"
    )

    qemu-system-x86_64 -name "${hostname}" "${qemuopts[@]}" "${hostqemuopts[@]}" "${opts[@]}" &
    wait_for_ssh

    # shellcheck disable=SC2087
    ssh -tt "${sshopts[@]}" <<EOF || true
      mkdir /var/cache/pacman-cache-host
      mount -t 9p -o trans=virtio,version=9p2000.L,ro pacman-cache /var/cache/pacman-cache-host

      # Uncomment CacheDir and prepend the host pacman cache as cachedir
      # At worst, the cache directory will be ignored if it does not exist
      # Pacman will always use the first directory with write access for downloads
      sed -i 's/^#\?\(CacheDir.*\)/\1\nCacheDir = \/var\/cache\/pacman-cache-host\//' /etc/pacman.conf

      mkdir /repo/
      mount /dev/disk/by-label/REPO /repo/

      printf 'lukspw\nlukspw\nrootpw\nrootpw\n' | \
        /repo/install_scripts/"${hostname}".sh

      mount /dev/mapper/vgbase-root /mnt

      cat << SPECIALS > /tmp/specials.sh
        if [[ "\\\$(tty)" == "/dev/tty1" ]] ; then
          mkdir /var/cache/pacman-cache-host
          mount -t 9p -o trans=virtio,version=9p2000.L,ro pacman-cache /var/cache/pacman-cache-host

          # Uncomment CacheDir and prepend the host pacman cache as cachedir
          # At worst, the cache directory will be ignored if it does not exist
          # Pacman will always use the first directory with write access for downloads
          sed -i 's/^#\?\(CacheDir.*\)/\1\nCacheDir = \/var\/cache\/pacman-cache-host\//' /etc/pacman.conf
        fi
SPECIALS

      mv /mnt/root/.bash_profile /tmp/rest.sh

      cat /tmp/specials.sh /tmp/rest.sh > /mnt/root/.bash_profile

      rsync -rl /repo/ /mnt/var/lib/dotfiles/

      umount /mnt

      poweroff
EOF

    wait
}

configure_new_system() {
    local hostname="${1}"
    shift
    local hostqemuopts=("${@}")

    opts=(
        "-fsdev" "local,id=pacman-cache,path=share,path=/var/cache/pacman/pkg/,readonly=on,security_model=none"
        "-device" "virtio-9p-pci,fsdev=pacman-cache,mount_tag=pacman-cache"

        "-monitor" "unix:${mon_sock},server=on,wait=off"
    )

    qemu-system-x86_64 -name "${hostname}" "${qemuopts[@]}" "${hostqemuopts[@]}" "${opts[@]}" &

    # 5s for grub timeout, 5s for kernel boot
    echo waiting for luks password prompt ...
    sleep 10s
    echo 'lukspw' | send_mon "${mon_sock}"

    echo waiting for boot ...
    sleep 10s
    wait
}

machines=(ares neptune dionysus)
if (($# > 0)); then
    machines=("${@}")
fi

download_iso

for hostname in "${machines[@]}"; do
    case "${hostname}" in
    ares)
        hostqemuopts=(
            "-device" "ide-hd,drive=root"
            "-drive" "if=pflash,format=raw,readonly=true,file=/usr/share/edk2/x64/OVMF_CODE.4m.fd"
            "-drive" "if=pflash,format=raw,file=${tmpdir}/efivars.fd"
        )
        ;;
    neptune)
        hostqemuopts=(
            "-device" "nvme,serial=rootnvme,drive=root"
            "-drive" "if=pflash,format=raw,readonly=true,file=/usr/share/edk2/x64/OVMF_CODE.4m.fd"
            "-drive" "if=pflash,format=raw,file=${tmpdir}/efivars.fd"
        )
        ;;
    dionysus)
        hostqemuopts=(
            "-device" "nvme,serial=rootnvme,drive=root"
            "-drive" "if=pflash,format=raw,readonly=true,file=/usr/share/edk2/x64/OVMF_CODE.4m.fd"
            "-drive" "if=pflash,format=raw,file=${tmpdir}/efivars.fd"
        )
        ;;
    *)
        printf "unknown hostname: %s\n" "${hostname}" >&2
        exit 1
        ;;
    esac
    [[ ! "${hostqemuopts[*]}" ]] && exit 1
    install_from_iso "${hostname}" "${hostqemuopts[@]}"
    configure_new_system "${hostname}" "${hostqemuopts[@]}"
done
