#!/usr/bin/env bash
# Funções compartilhadas. Não executar diretamente: é carregado pelo install.sh.

log() { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merro:\033[0m %s\n' "$*" >&2; exit 1; }

# DRY_RUN=1 só imprime o comando. Não serve para redirecionamentos ou pipes.
run() {
    if [[ ${DRY_RUN:-0} == 1 ]]; then
        printf '[dry-run] %s\n' "$*"
    else
        "$@"
    fi
}
