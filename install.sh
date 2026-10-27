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

DISK=${1:-}
DRY_RUN=0
[[ ${2:-} == --dry-run ]] && DRY_RUN=1
export DRY_RUN

[[ -n $DISK ]] || die "uso: $0 /dev/sdX [--dry-run]"
[[ $DRY_RUN == 1 || $EUID -eq 0 ]] || die "precisa de root (ou use --dry-run)"

[[ $DRY_RUN == 1 ]] || require_host_tools
check_target "$DISK"
confirm_target "$DISK"

# Desmonta /mnt se algo der errado no meio (no dry-run não mexe em nada)
[[ $DRY_RUN == 1 ]] || trap 'umount -R /mnt 2>/dev/null || true' EXIT

log "particionando $DISK"
partition_disk "$DISK"
format_disk "$DISK"
mount_layout "$DISK"

log "disco pronto em /mnt"
