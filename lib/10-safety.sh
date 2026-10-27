#!/usr/bin/env bash
# Recusa qualquer alvo que possa ser o disco do sistema. Carregado pelo install.sh.

# Discos físicos por trás de todos os sistemas de arquivos montados neste host.
# Falha fechada: se não conseguir listar, o chamador aborta.
host_disks() {
    local src
    while read -r src; do
        src=${src%%\[*}  # btrfs mostra subvolume assim: /dev/mapper/x[/@]
        [[ $src == /dev/* ]] || continue
        lsblk -srno NAME,TYPE "$src" | awk '$2 == "disk" { print $1 }'
    done < <(findmnt -rno SOURCE) | sort -u
}

check_target() {
    local disk=$1 type tran name hd
    type=$(lsblk -dno TYPE "$disk" 2>/dev/null) || die "$disk não é um dispositivo"
    read -r type tran < <(lsblk -dno TYPE,TRAN "$disk")
    [[ $type == disk ]] || die "$disk não é um disco inteiro (é '$type')"
    [[ $tran == usb ]] || die "$disk não é USB (transporte '$tran'); recuso por segurança"

    name=$(lsblk -dno NAME "$disk")
    hd=$(host_disks) || die "não consegui listar os discos do sistema"
    grep -qx "$name" <<< "$hd" && die "$disk é um disco do sistema atual; recuso"

    [[ -z $(lsblk -rno MOUNTPOINTS "$disk" | sed '/^$/d') ]] || die "$disk tem partições montadas; desmonte antes"
}

confirm_target() {
    local disk=$1 answer
    lsblk -o NAME,SIZE,MODEL,FSTYPE,MOUNTPOINTS "$disk"
    [[ ${DRY_RUN:-0} == 1 ]] && return 0
    read -rp "Isso APAGA TODO o conteúdo de $disk. Digite o caminho exato para continuar: " answer
    [[ $answer == "$disk" ]] || die "confirmação não bateu; nada foi feito"
}
