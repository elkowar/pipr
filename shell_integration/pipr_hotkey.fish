#!/bin/fish
function _run_pipr_hotkey
  set -l commandline (commandline -b)
  pipr --out-file /tmp/pipr_out --default "$commandline" > /dev/null
  set -l result (cat /tmp/pipr_out)
  commandline -r $result
  commandline -f repaint
end

bind \ca _run_pipr_hotkey

