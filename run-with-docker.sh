#!/usr/bin/env bash

set -euo pipefail

docker build --tag 'prime-ble-firmware-build' --progress=plain .
docker run \
    --rm \
    -it \
    --mount "type=bind,\"source=$PWD\",\"destination=$PWD\"" \
    --workdir "$PWD" \
    'prime-ble-firmware-build' \
    $@
