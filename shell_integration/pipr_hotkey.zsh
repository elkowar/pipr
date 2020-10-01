#!/usr/bin/env zsh

_pipr_expand_widget() {
  emulate -LR zsh
  </dev/tty pipr --out-file /tmp/pipr_out --default "$LBUFFER" >/dev/null
  LBUFFER=$(< /tmp/pipr_out)
}
zle -N _pipr_expand_widget
bindkey '\ea' _pipr_expand_widget
