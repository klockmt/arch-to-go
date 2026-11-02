#!/bin/sh
# Source this file: exports every variable from .env (never printed).
set -a
# shellcheck disable=SC1091
. "$(dirname "$0")/../.env"
set +a
