#!/usr/bin/env bash
# Instala Arch Linux num SSD USB. Uso: ./install.sh /dev/sdX [--dry-run]
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib/common.sh
source "$ROOT_DIR/lib/common.sh"
# shellcheck source=lib/10-safety.sh
source "$ROOT_DIR/lib/10-safety.sh"
# shellcheck source=lib/20-disk.sh
source "$ROOT_DIR/lib/20-disk.sh"
# shellcheck source=lib/30-base.sh
source "$ROOT_DIR/lib/30-base.sh"
# shellcheck source=lib/40-boot.sh
source "$ROOT_DIR/lib/40-boot.sh"
# shellcheck source=lib/50-desktop.sh
source "$ROOT_DIR/lib/50-desktop.sh"
# shellcheck source=lib/60-paru.sh
source "$ROOT_DIR/lib/60-paru.sh"

DISK=${1:-}
DRY_RUN=0
[[ ${2:-} == --dry-run ]] && DRY_RUN=1
export DRY_RUN

[[ -n $DISK ]] || die "uso: $0 /dev/sdX [--dry-run]"
[[ $DRY_RUN == 1 || $EUID -eq 0 ]] || die "precisa de root (ou use --dry-run)"

[[ $DRY_RUN == 1 ]] || require_host_tools
check_target "$DISK"
confirm_target "$DISK"
ask_account
log "compilando paru (ou usando o pacote já pronto)"
build_paru

# Desmonta /mnt se algo der errado no meio (no dry-run não mexe em nada)
[[ $DRY_RUN == 1 ]] || trap 'umount -R /mnt 2>/dev/null || true' EXIT

log "particionando $DISK"
partition_disk "$DISK"
format_disk "$DISK"
mount_layout "$DISK"

log "instalando sistema base"
install_base
configure_base

log "instalando Limine"
install_limine "$DISK"

log "instalando desktop e rede"
install_desktop

log "instalando paru"
install_paru

log "sistema instalado em /mnt"
