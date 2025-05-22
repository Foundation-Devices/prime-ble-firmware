#!/usr/bin/bash
set -euo pipefail

export TZ=America/New_York

# Switch to the user of the mounted directory.
# This could be UID=1000 for regular installs, 1000+ for multi-user setups,
# and UID=0 for remapped UIDs.
# Needed to make sure we don't create build files with root or alien uids.
PWD_UID=$(stat -c '%u' .)
PWD_GID=$(stat -c '%g' .)

sudo -u "#${PWD_UID}" -g "#${PWD_GID}" PATH="/root/.cargo/bin:${PATH}" HOME=/root $@
