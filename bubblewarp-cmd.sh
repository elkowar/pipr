#!/bin/bash
bwrap --ro-bind /usr /usr --symlink usr/lib64 /lib64 --tmpfs /tmp --proc /proc --dev /dev --ro-bind /etc /etc --ro-bind ./ /pipr-bin --bind /home/leon/.config/pipr /pipr_config --die-with-parent --share-net --unshare-pid "$@"

