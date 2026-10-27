#!/usr/bin/env bash
# Instala Arch Linux num SSD USB. Uso: ./install.sh /dev/sdX [--dry-run]
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# shellcheck source=lib/common.sh
source "$ROOT_DIR/lib/common.sh"

DISK=${1:-}
DRY_RUN=0
[[ ${2:-} == --dry-run ]] && DRY_RUN=1
export DRY_RUN

[[ -n $DISK ]] || die "uso: $0 /dev/sdX [--dry-run]"

log "esqueleto pronto para $DISK (dry-run=$DRY_RUN)"
