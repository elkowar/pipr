#!/bin/fish
function run_stuff
  set -l commandline (commandline -b)
  pipr --out-file /tmp/pipr_out --default "$commandline"
  set -l result (cat /tmp/pipr_out)
  commandline -r $result
  commandline -f repaint
end

bind \ca run_stuff

