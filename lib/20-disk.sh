#!/usr/bin/env bash
# Particiona, formata e monta o SSD em /mnt. Carregado pelo install.sh.

ESP_SIZE=${ESP_SIZE:-1G}
# nodatacow desliga CoW e checksums. Escolha do usuário, sem checar RAM.
BTRFS_OPTS=${BTRFS_OPTS:-noatime,ssd,discard=async,nodatacow}

require_host_tools() {
    local tool
    for tool in sgdisk wipefs partprobe udevadm mkfs.fat mkfs.btrfs btrfs; do
        command -v "$tool" >/dev/null || die "falta '$tool' no host (pacotes: gptfdisk, util-linux, dosfstools, btrfs-progs)"
    done
}

# part <disco> <n>: /dev/sdb1 e /dev/nvme0n1p1, conforme o nome termina em dígito
part() {
    if [[ $1 =~ [0-9]$ ]]; then
        echo "${1}p$2"
    else
        echo "${1}$2"
    fi
}

partition_disk() {
    local disk=$1
    run wipefs --all --force "$disk"
    run sgdisk --zap-all "$disk"
    run sgdisk -n1:0:+"$ESP_SIZE" -t1:ef00 -c1:ESP "$disk"
    run sgdisk -n2:0:0 -t2:8300 -c2:archroot "$disk"
    run partprobe "$disk"
    run udevadm settle
}

format_disk() {
    local disk=$1
    run mkfs.fat -F32 -n ESP "$(part "$disk" 1)"
    run mkfs.btrfs -f -L archroot "$(part "$disk" 2)"
}

# Subvolumes: @ raiz, @home, @log e @pkg (cache do pacman, fica fora do snapshot da raiz)
mount_layout() {
    local disk=$1 root esp sv
    root=$(part "$disk" 2)
    esp=$(part "$disk" 1)

    run mount "$root" /mnt
    for sv in @ @home @log @pkg; do
        run btrfs subvolume create "/mnt/$sv"
    done
    run umount /mnt

    run mount -o "subvol=@,$BTRFS_OPTS" "$root" /mnt
    run mkdir -p /mnt/home /mnt/var/log /mnt/var/cache/pacman/pkg /mnt/boot
    run mount -o "subvol=@home,$BTRFS_OPTS" "$root" /mnt/home
    run mount -o "subvol=@log,$BTRFS_OPTS" "$root" /mnt/var/log
    run mount -o "subvol=@pkg,$BTRFS_OPTS" "$root" /mnt/var/cache/pacman/pkg
    run mount "$esp" /mnt/boot
}
