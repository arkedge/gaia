#!/bin/bash

set -Cue

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

function get_wsl_host_addr() {
  if ! grep -qEi "(microsoft|wsl)" /proc/version; then
    return 0
  fi
  WSL_HOST_ADDR=$(ip route | grep 'default via' | grep -Eo '[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}')
}

if [ -z "${WSL_HOST_ADDR+x}" ]; then
    get_wsl_host_addr
fi

# ポート名の環境変数が定義されていない時には空文字に潰す
# jsonnet 側で空文字の場合には無視するようにしている
jrsonnet \
  -V KBLE_SERIALPORT_ADDR="${WSL_HOST_ADDR:-127.0.0.1}" \
  -V PORT_CH_RS422_MIS_IF="${PORT_CH_RS422_MIS_IF:-}" \
  -V PORT_CH_UART_TEST="${PORT_CH_UART_TEST:-}" \
  -V PORT_CH_LVTTL_AOBC="${PORT_CH_LVTTL_AOBC:-}" \
  -V PORT_CH_LVTTL_TOBC="${PORT_CH_LVTTL_TOBC:-}" \
  -V PORT_CH_LVTTL_STX="${PORT_CH_LVTTL_STX:-}" \
  -V PORT_CH_RS422_MOBC_EXT="${PORT_CH_RS422_MOBC_EXT:-}" \
  -V PORT_CH_RS422_XTX="${PORT_CH_RS422_XTX:-}" \
  -V PORT_CH_RS422_LCT="${PORT_CH_RS422_LCT:-}" \
  -V PORT_CH_RS422_MISSION2="${PORT_CH_RS422_MISSION2:-}" \
  -V PORT_CH_RS422_MISSION3="${PORT_CH_RS422_MISSION3:-}" \
  -V PORT_CH_LVTTL_LOBC="${PORT_CH_LVTTL_LOBC:-}" \
  -V PORT_CH_LVTTL_MISSION4="${PORT_CH_LVTTL_MISSION4:-}" \
  -V PORT_CH_LVTTL_PCDU="${PORT_CH_LVTTL_PCDU:-}" \
  -V PORT_CH_RS422_MOBC_EXT_SUB="${PORT_CH_RS422_MOBC_EXT_SUB:-}" \
  "$SCRIPT_DIR/spaghetti.${KBLE_ENV:-sils}.jsonnet"
