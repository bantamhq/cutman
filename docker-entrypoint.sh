#!/bin/sh
set -e

if [ "$#" -gt 0 ] && [ "${1#-}" = "$1" ]; then
    exec cutman "$@"
fi

if [ ! -f "$CUTMAN_DATA_DIR/.admin_token" ]; then
    cutman admin init --data-dir "$CUTMAN_DATA_DIR" --non-interactive
fi

set -- serve --data-dir "$CUTMAN_DATA_DIR" --host 0.0.0.0 --port "$CUTMAN_PORT" "$@"

if [ -n "$CUTMAN_PUBLIC_BASE_URL" ]; then
    set -- "$@" --public-base-url "$CUTMAN_PUBLIC_BASE_URL"
fi

exec cutman "$@"
