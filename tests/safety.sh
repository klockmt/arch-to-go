#!/usr/bin/env bash
# Testa lib/10-safety.sh com lsblk e findmnt mockados. Uso: bash tests/safety.sh
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
MOCK_BIN=$(mktemp -d)
trap 'rm -rf "$MOCK_BIN"' EXIT

cat > "$MOCK_BIN/lsblk" <<'EOF'
#!/usr/bin/env bash
[[ ${MOCK_FAIL:-0} == 1 ]] && exit 1
case "$*" in
    *"-dno TYPE,TRAN"*) echo "$MOCK_TYPE $MOCK_TRAN" ;;
    *"-dno TYPE"*)      echo "$MOCK_TYPE" ;;
    *"-dno NAME"*)      basename "${@: -1}" ;;
    *"-rno MOUNTPOINTS"*) printf '%s\n' "$MOCK_MOUNTS" ;;
    *"-srno NAME,TYPE"*) printf '%s\n' "$MOCK_ANCESTORS" ;;
esac
EOF

cat > "$MOCK_BIN/findmnt" <<'EOF'
#!/usr/bin/env bash
printf '%s\n' "$MOCK_SOURCES"
EOF

chmod +x "$MOCK_BIN"/*
export PATH="$MOCK_BIN:$PATH"

# shellcheck source=lib/common.sh
source "$ROOT_DIR/lib/common.sh"
# shellcheck source=lib/10-safety.sh
source "$ROOT_DIR/lib/10-safety.sh"

fails=0

# expect <ok|refuse> <nome> <dispositivo>; o estado vem das variáveis MOCK_*
expect() {
    local want=$1 name=$2 dev=$3 got=ok
    (check_target "$dev") >/dev/null 2>&1 || got=refuse
    if [[ $got == "$want" ]]; then
        echo "ok    $name"
    else
        echo "FAIL  $name (esperado $want, veio $got)"
        fails=$((fails + 1))
    fi
}

export MOCK_SOURCES=$'/dev/mapper/luks-x\n/dev/nvme0n1p1\n/dev/mapper/luks-x'
export MOCK_ANCESTORS=$'luks-x crypt\nnvme0n1p2 part\nnvme0n1 disk'

export MOCK_FAIL=0 MOCK_TYPE=disk MOCK_TRAN=usb MOCK_MOUNTS=''
expect ok "USB limpo, sem relação com o host" /dev/sdb

export MOCK_TYPE=disk MOCK_TRAN=usb MOCK_MOUNTS=''
expect refuse "disco do sistema (nvme0n1)" /dev/nvme0n1

export MOCK_TYPE=disk MOCK_TRAN=nvme
expect refuse "não é USB (nvme)" /dev/sdb

export MOCK_TYPE=part MOCK_TRAN=usb
expect refuse "partição em vez de disco inteiro" /dev/sdb1

export MOCK_TYPE=disk MOCK_TRAN=usb MOCK_MOUNTS='/mnt/x'
expect refuse "disco com partição montada" /dev/sdb

export MOCK_FAIL=1
expect refuse "dispositivo inexistente" /dev/nope
export MOCK_FAIL=0

if (( fails > 0 )); then
    echo "$fails teste(s) falharam"
    exit 1
fi
echo "todos os testes passaram"
