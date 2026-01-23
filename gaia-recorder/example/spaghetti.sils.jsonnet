// src/src_user/settings/port_config.h
local PORT_CONFIG = {
  PORT_CH_RS422_MIS_IF: 0,
  PORT_CH_UART_TEST: 1,
  PORT_CH_LVTTL_AOBC: 2,
  PORT_CH_LVTTL_TOBC: 3,
  PORT_CH_LVTTL_STX: 4,
  PORT_CH_RS422_MOBC_EXT: 5,
  PORT_CH_RS422_XTX: 6,
  PORT_CH_RS422_LCT: 7,
  PORT_CH_RS422_MISSION2: 8,
  PORT_CH_RS422_MISSION3: 9,
  PORT_CH_LVTTL_LOBC: 10,
  PORT_CH_LVTTL_MISSION4: 11,
  PORT_CH_LVTTL_PCDU: 12,
  PORT_CH_RS422_MOBC_EXT_SUB: 13,
};

local uart_spaghetti = std.foldl(function(a, b) a + b, [
  {
    plugs+: {
      // SILS 側の kble socket を開く
      ['sils_' + portName]: 'ws://localhost:9696/channels/%s' % PORT_CONFIG[portName],
      // PC 側のシリアルポートを開く
      ['pc_' + portName]: 'ws://%s:9600/open?baudrate=115200&port=%s' % [
        std.extVar('KBLE_SERIALPORT_ADDR'),
        std.extVar(portName),
      ],
    },
    links+: {
      // 双方向に接続
      ['sils_' + portName]: 'pc_' + portName,
      ['pc_' + portName]: 'sils_' + portName,
    },
  }
  for portName in std.objectFields(PORT_CONFIG)
  if std.extVar(portName) != '' // 空文字列の場合は接続しない
], {});

uart_spaghetti + {
  plugs+: {
    tmtc_c2a: 'ws://localhost:8910',
    sils_ccsds: 'ws://localhost:22545',
  },
  links+: {
    tmtc_c2a: 'sils_ccsds',  // CMD
    sils_ccsds: 'tmtc_c2a',  // TLM
  },
}
