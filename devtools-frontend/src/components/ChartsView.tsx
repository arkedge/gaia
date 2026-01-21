import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Button, Classes, Icon, InputGroup, Switch } from "@blueprintjs/core";
import { IconNames } from "@blueprintjs/icons";
import { useClient } from "./Layout";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";

const RECORDER_ENDPOINT_KEY = "c2a-devtools-recorder-endpoint";
const PANEL_SERIES_KEY = "c2a-devtools-panel-series";
const SERIES_PICKER_KEY = "c2a-devtools-series-picker";

type RecordingSession = {
  session_id: string;
  suffix: string;
  started_at_ms: number;
  db_path: string;
  active: boolean;
};

type RecordingListItem = {
  session_id: string;
  suffix: string;
  started_at_ms: number | null;
  db_path: string;
};

type TelemetrySample = {
  time_ms: number;
  value_type: string;
  value_num: number | null;
  value_int: number | null;
  value_text: string | null;
  value_bytes_hex: string | null;
};

type CommandLogItem = {
  time_ms: number;
  command_name: string;
  params_json: string;
};

type SeriesConfig = {
  id: string;
  tmivName: string;
  fieldName: string;
  isRaw: boolean;
  color: string;
};

type SeriesDataMap = Map<string, TelemetrySample[]>;

const buildDefaultRecorderEndpoint = () => {
  const url = new URL(window.location.origin);
  url.port = "8920";
  return url.toString().replace(/\/$/, "");
};

const getRecorderEndpoint = () => {
  return (
    localStorage.getItem(RECORDER_ENDPOINT_KEY) ??
    buildDefaultRecorderEndpoint()
  );
};

const setRecorderEndpoint = (value: string) => {
  localStorage.setItem(RECORDER_ENDPOINT_KEY, value);
};

const getPanelSeriesState = () => {
  const stored = localStorage.getItem(PANEL_SERIES_KEY);
  if (!stored) return null;
  try {
    return JSON.parse(stored) as {
      panelA: SeriesConfig[];
      panelB: SeriesConfig[];
    };
  } catch {
    return null;
  }
};

const setPanelSeriesState = (panelA: SeriesConfig[], panelB: SeriesConfig[]) => {
  localStorage.setItem(
    PANEL_SERIES_KEY,
    JSON.stringify({ panelA, panelB })
  );
};

const getSeriesPickerState = () => {
  const stored = localStorage.getItem(SERIES_PICKER_KEY);
  if (!stored) return null;
  try {
    return JSON.parse(stored) as {
      selectedTmiv: string;
      selectedField: string;
      selectedRaw: boolean;
    };
  } catch {
    return null;
  }
};

const setSeriesPickerState = (tmiv: string, field: string, raw: boolean) => {
  localStorage.setItem(
    SERIES_PICKER_KEY,
    JSON.stringify({ selectedTmiv: tmiv, selectedField: field, selectedRaw: raw })
  );
};

const palette = ["#38bdf8", "#f97316", "#22c55e", "#e11d48", "#a855f7"];
const stateColorPalette = [
  "#3b82f6", // blue
  "#f97316", // orange
  "#10b981", // green
  "#ef4444", // red
  "#8b5cf6", // purple
  "#ec4899", // pink
  "#f59e0b", // amber
  "#06b6d4", // cyan
  "#84cc16", // lime
  "#6366f1", // indigo
];
const axisStroke = "rgba(226, 232, 240, 0.9)";
const gridStroke = "rgba(148, 163, 184, 0.2)";

// Debug log buffer (max 1000 entries)
const debugLogBuffer: string[] = [];
const MAX_DEBUG_LOGS = 1000;

const debugLog = (message: string) => {
  const timestamp = new Date().toISOString();
  const logEntry = `${timestamp} ${message}`;
  console.log(message);
  debugLogBuffer.push(logEntry);
  if (debugLogBuffer.length > MAX_DEBUG_LOGS) {
    debugLogBuffer.shift();
  }
};

const downloadDebugLogs = () => {
  const content = debugLogBuffer.join('\n');
  const blob = new Blob([content], { type: 'text/plain' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `charts-debug-${new Date().toISOString().replace(/[:.]/g, '-')}.log`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
};

const clearDebugLogs = () => {
  debugLogBuffer.length = 0;
  console.clear();
};

const buildCategoricalRange = (values: number[]) => {
  if (values.length === 0) {
    return [-1, 1];
  }
  const min = Math.min(...values);
  const max = Math.max(...values);
  if (min === max) {
    return [min - 1, max + 1];
  }
  return [min - 0.5, max + 0.5];
};

// Helper: Get state color based on state value
const getStateColor = (stateValue: number): string => {
  return stateColorPalette[stateValue % stateColorPalette.length];
};

const buildGlobalStateMapping = (
  series: SeriesConfig[],
  maps: Map<string, Map<string, number>>,
) => {
  const labelToValue = new Map<string, number>();
  for (const s of series) {
    const local = maps.get(s.id);
    if (!local) {
      continue;
    }
    for (const label of local.keys()) {
      if (!labelToValue.has(label)) {
        labelToValue.set(label, labelToValue.size);
      }
    }
  }
  return labelToValue;
};

const mapValueToLabel = (map: Map<number, string>, value: number) => {
  const exact = map.get(value);
  if (exact !== undefined) {
    return exact;
  }
  const rounded = map.get(Math.round(value));
  if (rounded !== undefined) {
    return rounded;
  }
  return map.get(Math.floor(value)) ?? map.get(Math.ceil(value)) ?? "";
};

const fetchJson = async <T,>(url: string, init?: RequestInit): Promise<T> => {
  const res = await fetch(url, init);
  if (!res.ok) {
    throw new Error(`request failed: ${res.status}`);
  }
  return (await res.json()) as T;
};

const buildSeriesId = (tmivName: string, fieldName: string, isRaw: boolean) =>
  `${tmivName}:${fieldName}:${isRaw ? "raw" : "conv"}`;

// Convert field name from schema format (SH_TI) to database format (SH.TI:conv or SH.TI:raw)
const convertFieldNameForQuery = (fieldName: string, isRaw: boolean): string => {
  const converted = fieldName.replace(/_/g, ".");
  return `${converted}:${isRaw ? "raw" : "conv"}`;
};

// Calculate max_points based on time range for dynamic sampling
// Narrow time ranges get more detail, wider ranges get downsampled
const calculateMaxPoints = (timeRangeMs: number): number => {
  const seconds = timeRangeMs / 1000;

  if (seconds <= 60) return 60000;     // 1 min or less: ~1 point per second
  if (seconds <= 300) return 30000;    // 5 min: high detail
  if (seconds <= 600) return 20000;    // 10 min: high detail
  if (seconds <= 1800) return 10000;   // 30 min: medium detail
  if (seconds <= 3600) return 5000;    // 1 hour: medium detail
  if (seconds <= 7200) return 3000;    // 2 hours: normal detail
  return 2000;                         // > 2 hours: lower detail
};

const buildQueryParams = (
  params: Record<string, string | number | boolean | undefined>,
) => {
  const query = new URLSearchParams();
  Object.entries(params).forEach(([key, value]) => {
    if (typeof value !== "undefined") {
      query.set(key, String(value));
    }
  });
  return query.toString();
};

const ensureNumber = (sample: TelemetrySample): number | null => {
  if (sample.value_num !== null) {
    return sample.value_num;
  }
  if (sample.value_int !== null) {
    return sample.value_int;
  }
  return null;
};

type ChartPanelProps = {
  title: string;
  series: SeriesConfig[];
  seriesData: SeriesDataMap;
  commands: CommandLogItem[];
  showCommands: boolean;
  follow: boolean;
  rangeMinutes: number;
  manualTimeRange?: { startMs: number; endMs: number } | null;
  onToggleShowCommands: () => void;
  onRemoveSeries: (id: string) => void;
  onToggleSeriesMode: (id: string) => void;
  onZoom: (startSec: number, endSec: number) => void;
  onResetZoom: () => void;
  valueMapsRef: React.MutableRefObject<Map<string, Map<string, number>>>;
};

const ChartPanel: React.FC<ChartPanelProps> = ({
  title,
  series,
  seriesData,
  commands,
  showCommands,
  follow,
  rangeMinutes,
  manualTimeRange,
  onToggleShowCommands,
  onRemoveSeries,
  onToggleSeriesMode,
  onZoom,
  onResetZoom,
  valueMapsRef,
}) => {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const plotRef = useRef<uPlot | null>(null);
  const commandsRef = useRef<CommandLogItem[]>(commands);
  const showCommandsRef = useRef<boolean>(showCommands);
  const seriesKeyRef = useRef<string>("");
  // Store time->label mapping for each series to use during drawing
  const timeLabelMapsRef = useRef<Map<string, Map<number, string>>>(new Map());

  // Helper: Collect text labels from samples
  const collectTextLabels = useCallback((
    series: SeriesConfig[],
    seriesData: SeriesDataMap,
  ) => {
    const localMaps = new Map<string, Map<string, number>>();
    for (const s of series) {
      const samples = seriesData.get(s.id) ?? [];
      const existingMap = valueMapsRef.current.get(s.id) ?? new Map();
      const textMap = new Map(existingMap);

      let textSampleCount = 0;
      for (const sample of samples) {
        if (sample.value_text && !textMap.has(sample.value_text)) {
          textMap.set(sample.value_text, 0);
          textSampleCount++;
        }
      }

      debugLog(`[collectTextLabels] series=${s.id}: total samples=${samples.length}, unique text values=${textMap.size}, new values found=${textSampleCount}`);

      localMaps.set(s.id, textMap);
    }
    return localMaps;
  }, [valueMapsRef]);

  // Helper: Build plot data for a single series
  const buildSeriesPlotData = useCallback((
    s: SeriesConfig,
    samples: TelemetrySample[],
    labelMap: Map<string, number> | undefined,
  ): { times: Set<number>; dataMap: Map<number, number | null> } => {
    const times = new Set<number>();
    const dataMap = new Map<number, number | null>();

    let numericCount = 0;
    let textCount = 0;
    let mappedCount = 0;

    for (const sample of samples) {
      const timeSec = sample.time_ms / 1000;
      const numeric = ensureNumber(sample);

      if (numeric !== null) {
        dataMap.set(timeSec, numeric);
        times.add(timeSec);
        numericCount++;
      } else if (sample.value_text && labelMap) {
        textCount++;
        const yValue = labelMap.get(sample.value_text);
        if (yValue !== undefined) {
          dataMap.set(timeSec, yValue);
          times.add(timeSec);
          mappedCount++;
        }
      }
    }

    debugLog(`[buildSeriesPlotData] series=${s.id}: samples=${samples.length}, numeric=${numericCount}, text=${textCount}, mapped=${mappedCount}, final points=${times.size}`);

    return { times, dataMap };
  }, []);

  const buildPlotDataAndMaps = useCallback(() => {
    // Collect all text labels
    const localMaps = collectTextLabels(series, seriesData);
    const globalStateMap = buildGlobalStateMapping(series, localMaps);
    const categoricalSeries = series.filter(
      (s) => (localMaps.get(s.id)?.size ?? 0) > 0,
    );
    const hasNumericSeries = categoricalSeries.length !== series.length;

    // Build y-position mapping for each series
    const yPositionMaps = new Map<string, Map<string, number>>();
    for (const s of series) {
      const textMap = localMaps.get(s.id);
      if (!textMap || textMap.size === 0) continue;

      const seriesLabelMap = new Map<string, number>();
      const seriesIndex = categoricalSeries.findIndex((c) => c.id === s.id);

      for (const [label] of textMap) {
        // Always use series index for Y position in state-only mode
        const yPosition = !hasNumericSeries ? seriesIndex : (globalStateMap.get(label) ?? 0);
        seriesLabelMap.set(label, yPosition);
      }
      yPositionMaps.set(s.id, seriesLabelMap);
    }

    // Build plot data and time->label mappings
    const allTimes = new Set<number>();
    const seriesDataMaps = new Map<string, Map<number, number | null>>();
    const timeLabelMaps = new Map<string, Map<number, string>>();

    for (const s of series) {
      const samples = seriesData.get(s.id) ?? [];
      const labelMap = yPositionMaps.get(s.id);
      const { times, dataMap } = buildSeriesPlotData(s, samples, labelMap);

      times.forEach(t => allTimes.add(t));
      seriesDataMaps.set(s.id, dataMap);

      // Build time->label mapping for this series
      const timeLabelMap = new Map<number, string>();
      for (const sample of samples) {
        const timeSec = sample.time_ms / 1000;
        if (sample.value_text) {
          timeLabelMap.set(timeSec, sample.value_text);
        }
      }
      timeLabelMaps.set(s.id, timeLabelMap);
    }

    // Convert to array format
    const times = Array.from(allTimes).sort((a, b) => a - b);
    const data: (number | null)[][] = [times];
    for (const s of series) {
      const map = seriesDataMaps.get(s.id) ?? new Map();
      data.push(times.map((t) => map.get(t) ?? null));
    }

    // Build final valueMaps with state value mappings
    const valueMaps = new Map<string, Map<string, number>>();
    for (const s of series) {
      const textMap = localMaps.get(s.id);
      if (textMap && textMap.size > 0) {
        const labelMap = new Map<string, number>();
        for (const [label] of textMap) {
          const globalValue = globalStateMap.get(label);
          if (globalValue !== undefined) {
            labelMap.set(label, globalValue);
          }
        }
        valueMaps.set(s.id, labelMap);
      } else {
        valueMaps.set(s.id, new Map());
      }
    }
    return { data, valueMaps, timeLabelMaps };
  }, [series, seriesData, collectTextLabels, buildSeriesPlotData]);

  const buildSeriesOptions = useCallback(() => {
    const categoricalSeries = series.filter(
      (s) => (valueMapsRef.current.get(s.id)?.size ?? 0) > 0,
    );
    const hasNumericSeries = categoricalSeries.length !== series.length;
    return [
      {
        // X軸（時間）のツールチップ表示をミリ秒精度でフォーマット
        value: (_self: any, rawValue: number) => {
          console.log('[X-axis value] rawValue:', rawValue);
          const date = new Date(rawValue * 1000);
          const formatted = date.toLocaleString('ja-JP', {
            year: 'numeric',
            month: '2-digit',
            day: '2-digit',
            hour: '2-digit',
            minute: '2-digit',
            second: '2-digit',
            fractionalSecondDigits: 3,
            hour12: false
          });
          console.log('[X-axis value] formatted:', formatted);
          return formatted;
        }
      } as uPlot.Series,
      ...series.map((s) => {
        const isCategorical = (valueMapsRef.current.get(s.id)?.size ?? 0) > 0;
        return {
          label: `${s.tmivName}.${s.fieldName}${s.isRaw ? "@RAW" : ""}`,
          stroke: isCategorical ? "transparent" : s.color,
          width: s.isRaw ? 1.5 : 2.5,
          paths: isCategorical ? () => null : (s.isRaw ? undefined : uPlot.paths.stepped),
          points: isCategorical ? { show: false } : { show: true, size: s.isRaw ? 2 : 3 },
          scale: isCategorical
            ? (hasNumericSeries ? `y_${s.id}` : "y")
            : "y",
        } as uPlot.Series;
      }),
    ];
  }, [series]);

  const buildAxesAndScales = useCallback(() => {
    const categoricalSeries = series.filter(
      (s) => (valueMapsRef.current.get(s.id)?.size ?? 0) > 0,
    );
    const hasNumericSeries = categoricalSeries.length !== series.length;
    // Y軸幅を統一: 両パネルのグラフ開始地点を揃えるため常に40px
    const yAxisSize = 40;
    const axes: uPlot.Axis[] = [
      {
        scale: "x",
        show: true,
        stroke: axisStroke,
        grid: { show: true, stroke: gridStroke },
        size: 46,
        font: "11px monospace",
        space: 80,  // 目盛り間の最小ピクセル数（重複を防ぐ）
        // 時刻表示の間隔を制御: ミリ秒精度で重複を防ぐ
        // incrs: 表示する時間間隔の候補（秒単位）
        incrs: [
          0.1,    // 100ms
          0.2,    // 200ms
          0.5,    // 500ms
          1,      // 1秒
          2,      // 2秒
          5,      // 5秒
          10,     // 10秒
          15,     // 15秒
          30,     // 30秒
          60,     // 1分
          120,    // 2分
          300,    // 5分
          600,    // 10分
          900,    // 15分
          1800,   // 30分
          3600,   // 1時間
        ],
        values: (u, vals) => {
          // 表示範囲から時間間隔を推定
          if (vals.length < 2) return vals.map(() => '');
          const timeRange = vals[vals.length - 1] - vals[0];
          const avgInterval = timeRange / (vals.length - 1);

          // 1秒未満の間隔ならミリ秒表示、それ以外は秒まで
          const showMilliseconds = avgInterval < 1;

          return vals.map((val) => {
            const date = new Date(val * 1000);
            if (showMilliseconds) {
              // ミリ秒精度: HH:MM:SS.mmm
              return date.toLocaleTimeString('ja-JP', {
                hour: '2-digit',
                minute: '2-digit',
                second: '2-digit',
                fractionalSecondDigits: 3,
                hour12: false
              });
            } else {
              // 秒精度: HH:MM:SS
              return date.toLocaleTimeString('ja-JP', {
                hour: '2-digit',
                minute: '2-digit',
                second: '2-digit',
                hour12: false
              });
            }
          });
        },
      },
      {
        scale: "y",
        show: !hasNumericSeries && categoricalSeries.length > 0 ? false : true, // Categoricalの場合は軸自体を非表示
        stroke: axisStroke,
        grid: { show: true, stroke: gridStroke },
        size: yAxisSize,
        font: "10px monospace",
        labelSize: 0, // Hide axis label space
        values: undefined, // ラベルは表示しない
        splits: undefined,
      },
    ];
    const scales: uPlot.Scales = {
      x: { time: true },
      y: { auto: true },
    };

    if (!hasNumericSeries && categoricalSeries.length > 0) {
      // Y values are always series indices (0, 1, 2, ...)
      const maxIdx = categoricalSeries.length - 1;
      scales.y = {
        auto: false,
        range: (_self, _dataMin, _dataMax) => [
          -0.5,
          maxIdx + 0.5,
        ],
      };
    }

    if (!hasNumericSeries) {
      return { axes, scales };
    }

    for (const s of categoricalSeries) {
      const map = valueMapsRef.current.get(s.id);
      if (!map || map.size === 0) {
        continue;
      }
      const values = Array.from(map.values());
      axes.push({
        scale: `y_${s.id}`,
        side: 2,
        show: false, // Hide axis - only show color markers
        stroke: axisStroke,
        grid: { show: false },
        values: () => [], // No text labels
        splits: () => [], // No splits
        size: 0,
        font: "11px monospace",
      });
      scales[`y_${s.id}`] = {
        auto: false,
        range: () => buildCategoricalRange(values),
      };
    }
    return { axes, scales };
  }, [series, valueMapsRef]);

  useEffect(() => {
    commandsRef.current = commands;
    showCommandsRef.current = showCommands;
    plotRef.current?.redraw();
  }, [commands, showCommands]);

  useEffect(() => {
    const handleResize = () => {
      if (!plotRef.current || !containerRef.current) {
        return;
      }
      const rect = containerRef.current.getBoundingClientRect();
      plotRef.current.setSize({
        width: Math.max(300, Math.floor(rect.width)),
        height: Math.max(240, Math.floor(rect.height)),
      });
    };

    window.addEventListener("resize", handleResize);
    return () => {
      window.removeEventListener("resize", handleResize);
      plotRef.current?.destroy();
      plotRef.current = null;
    };
  }, []);

  useEffect(() => {
    const { data, valueMaps, timeLabelMaps } = buildPlotDataAndMaps();
    const hasData = data[0].length > 0;
    if (!hasData) {
      if (plotRef.current) {
        plotRef.current.destroy();
        plotRef.current = null;
      }
      return;
    }
    if (!containerRef.current) {
      return;
    }

    // Build a key that includes both the series structure and the label content
    const nextSeriesKey = series
      .map((s) => {
        const map = valueMaps.get(s.id);
        const labels = map ? Array.from(map.keys()).sort().join(",") : "";
        return `${s.id}:${map?.size ?? 0}:${labels}`;
      })
      .join("|");

    // Always update valueMapsRef and timeLabelMapsRef before any axis operations
    valueMapsRef.current = valueMaps;
    timeLabelMapsRef.current = timeLabelMaps;

    if (seriesKeyRef.current !== nextSeriesKey && plotRef.current) {
      plotRef.current.destroy();
      plotRef.current = null;
    }
    seriesKeyRef.current = nextSeriesKey;
    if (!plotRef.current) {
      const { width, height } = containerRef.current.getBoundingClientRect();
      const { axes, scales } = buildAxesAndScales();
      const plot = new uPlot(
        {
          width: Math.max(300, Math.floor(width)),
          height: Math.max(240, Math.floor(height)),
          scales,
          series: buildSeriesOptions(),
          axes,
          cursor: {
            drag: { x: true, y: false },
            focus: { prox: 16 },
          },
          legend: {
            show: true,
            live: true,
          },
          hooks: {
            draw: [
              (u) => {
                const ctx = u.ctx;

                // Draw commands if enabled
                if (showCommandsRef.current) {
                  ctx.save();
                  ctx.strokeStyle = "rgba(251, 191, 36, 0.8)";
                  ctx.fillStyle = "rgba(251, 191, 36, 0.9)";
                  ctx.font = "11px monospace";
                  for (const cmd of commandsRef.current) {
                    const x = u.valToPos(cmd.time_ms / 1000, "x", true);
                    if (x < u.bbox.left || x > u.bbox.left + u.bbox.width) {
                      continue;
                    }
                    ctx.beginPath();
                    ctx.moveTo(x, u.bbox.top);
                    ctx.lineTo(x, u.bbox.top + u.bbox.height);
                    ctx.stroke();
                    ctx.fillText(cmd.command_name, x + 4, u.bbox.top + 12);
                  }
                  ctx.restore();
                }

                // Draw state bars with colors based on state value
                const xVals = u.data[0] as number[];
                const categoricalSeries = series.filter(
                  (s) => (valueMapsRef.current.get(s.id)?.size ?? 0) > 0,
                );
                const hasNumericSeries =
                  categoricalSeries.length !== series.length;

                // Draw bars for each series
                debugLog(`[ChartsView] Draw hook called. series count=${series.length}, xVals count=${xVals.length}`);

                // Save context and set clipping region for the plot area
                ctx.save();
                ctx.beginPath();
                ctx.rect(u.bbox.left, u.bbox.top, u.bbox.width, u.bbox.height);
                ctx.clip();

                for (const [idx, s] of series.entries()) {
                  const labelMap = valueMapsRef.current.get(s.id);
                  if (!labelMap || labelMap.size === 0) continue;

                  debugLog(`[ChartsView] Processing series ${idx}: id=${s.id}, labelMap size=${labelMap.size}, labels=${Array.from(labelMap.keys()).join(',')}`);

                  const scale = hasNumericSeries ? `y_${s.id}` : "y";
                  const yVals = u.data[idx + 1] as (number | null)[];
                  const barHeight = 16;

                  // Get time->label mapping for this series
                  const timeLabelMap = timeLabelMapsRef.current.get(s.id);

                  let drawnBars = 0;
                  let drawnTexts = 0;
                  let missingLabels = 0;

                  // First pass: Draw all bars
                  for (let i = 0; i < yVals.length; i++) {
                    const yVal = yVals[i];
                    if (yVal === null || yVal === undefined) continue;

                    // Look up the label using the time->label mapping
                    const stateLabel = timeLabelMap?.get(xVals[i]) ?? "";
                    const stateValue = labelMap.get(stateLabel) ?? 0;

                    const x0 = u.valToPos(xVals[i], "x", true);
                    const x1 = u.valToPos(xVals[i + 1] ?? xVals[i] + 1, "x", true);
                    const y = u.valToPos(yVal, scale, true);
                    const widthPx = Math.max(1, x1 - x0);

                    // Check if this bar is visible in the plot area
                    const isVisible = x1 >= u.bbox.left && x0 <= u.bbox.left + u.bbox.width;

                    // Debug first few and last few points
                    if (i < 3 || i >= yVals.length - 3) {
                      debugLog(`[Bar ${i}] time=${xVals[i].toFixed(1)}, series=${s.id}, hasTimeLabel=${!!timeLabelMap?.has(xVals[i])}, label="${stateLabel}", value=${stateValue}, widthPx=${widthPx.toFixed(1)}, x0=${x0.toFixed(1)}, y=${y.toFixed(1)}, isVisible=${isVisible}`);
                    }

                    // Draw colored bar
                    ctx.fillStyle = getStateColor(stateValue);
                    ctx.fillRect(x0, y - barHeight / 2, widthPx, barHeight);
                    drawnBars++;

                    if (!stateLabel) {
                      missingLabels++;
                    }
                  }

                  // Second pass: Draw all text labels on top of bars
                  let prevStateLabel = "";
                  let isFirstVisibleBar = true;

                  for (let i = 0; i < yVals.length; i++) {
                    const yVal = yVals[i];
                    if (yVal === null || yVal === undefined) continue;

                    // Look up the label using the time->label mapping
                    const stateLabel = timeLabelMap?.get(xVals[i]) ?? "";

                    const x0 = u.valToPos(xVals[i], "x", true);
                    const x1 = u.valToPos(xVals[i + 1] ?? xVals[i] + 1, "x", true);
                    const y = u.valToPos(yVal, scale, true);

                    // Check if this bar is visible in the plot area
                    const isVisible = x1 >= u.bbox.left && x0 <= u.bbox.left + u.bbox.width;

                    // Draw text at the start of each new state region OR for the first visible bar
                    const shouldDrawText = stateLabel && (stateLabel !== prevStateLabel || isFirstVisibleBar);

                    if (shouldDrawText && isVisible) {
                      ctx.fillStyle = "rgba(255, 255, 255, 0.95)";
                      ctx.font = "12px monospace";
                      ctx.textAlign = "left";
                      ctx.textBaseline = "middle";

                      // Calculate text width to check if it fits
                      const textMetrics = ctx.measureText(stateLabel);
                      const textWidth = textMetrics.width;

                      // Use the bar's start position if it's within the plot area
                      // Otherwise, use the left edge of the plot area
                      const textX = Math.max(x0 + 4, u.bbox.left + 2);

                      // Only draw if there's enough space for at least part of the text
                      const availableWidth = (u.bbox.left + u.bbox.width) - textX;
                      if (availableWidth > 20) {  // Minimum 20px to show meaningful text
                        ctx.fillText(stateLabel, textX, y);
                        drawnTexts++;

                        // Debug first few and last few text draws
                        if (i < 3 || i >= yVals.length - 3) {
                          debugLog(`[Text ${i}] Drew "${stateLabel}" at x=${textX.toFixed(1)}, y=${y.toFixed(1)}, x0=${x0.toFixed(1)}, bbox.left=${u.bbox.left.toFixed(1)}, textWidth=${textWidth.toFixed(1)}, availableWidth=${availableWidth.toFixed(1)}, isFirstVisible=${isFirstVisibleBar}`);
                        }
                      } else {
                        // Debug skipped text
                        if (i < 3 || i >= yVals.length - 3) {
                          debugLog(`[Text ${i}] Skipped "${stateLabel}" - insufficient space. x0=${x0.toFixed(1)}, textX=${textX.toFixed(1)}, textWidth=${textWidth.toFixed(1)}, availableWidth=${availableWidth.toFixed(1)}`);
                        }
                      }

                      if (isFirstVisibleBar) {
                        isFirstVisibleBar = false;
                      }
                      prevStateLabel = stateLabel;
                    }

                    // Update first visible bar flag
                    if (isVisible && isFirstVisibleBar && stateLabel) {
                      isFirstVisibleBar = false;
                    }
                  }

                  debugLog(`[ChartsView] Series ${s.id}: drew ${drawnBars} bars, ${drawnTexts} texts, ${missingLabels} points missing labels`);
                }

                // Restore context (removes clipping)
                ctx.restore();

                // Draw Y-axis color indicators for categorical series
                if (categoricalSeries.length > 0 && !hasNumericSeries) {
                  ctx.save();
                  const indicatorWidth = 8;
                  const indicatorHeight = 8;
                  const xPos = 12; // Y軸内で中央寄せ (40px幅の中央付近)

                  for (const [idx, s] of categoricalSeries.entries()) {
                    const scale = "y";
                    const yVal = idx;
                    const y = u.valToPos(yVal, scale, true);

                    // Draw colored rectangle as indicator
                    ctx.fillStyle = s.color;
                    ctx.fillRect(xPos, y - indicatorHeight / 2, indicatorWidth, indicatorHeight);

                    // Optional: Add border for visibility
                    ctx.strokeStyle = "rgba(255, 255, 255, 0.3)";
                    ctx.lineWidth = 1;
                    ctx.strokeRect(xPos, y - indicatorHeight / 2, indicatorWidth, indicatorHeight);
                  }
                  ctx.restore();
                }
              },
            ],
            setSelect: [
              (u) => {
                if (u.select.width <= 0) {
                  return;
                }
                const min = u.posToVal(u.select.left, "x");
                const max = u.posToVal(u.select.left + u.select.width, "x");
                u.setScale("x", { min, max });
                u.setSelect({ left: 0, top: 0, width: 0, height: 0 });
                // Call onZoom to sync the other panel
                onZoom(min, max);
              },
            ],
          },
        },
        data as uPlot.AlignedData,
        containerRef.current,
      );
      plotRef.current = plot;
    }
    console.log('[ChartPanel] follow:', follow, 'manualTimeRange:', manualTimeRange);
    plotRef.current.setData(data as uPlot.AlignedData);
    if (manualTimeRange) {
      // Use manual time range from slider or zoom
      const startSec = manualTimeRange.startMs / 1000;
      const endSec = manualTimeRange.endMs / 1000;
      console.log('[ChartPanel] Using manual time range:', startSec, '-', endSec);
      plotRef.current.setScale("x", { min: startSec, max: endSec });
    } else if (follow) {
      const endSec = Date.now() / 1000;
      const startSec = endSec - rangeMinutes * 60;
      console.log('[ChartPanel] Using follow mode:', startSec, '-', endSec);
      plotRef.current.setScale("x", { min: startSec, max: endSec });
    } else {
      // Auto-scale to data range (e.g., after reset in playback mode or when no zoom/slider active)
      const xData = data[0] as number[];
      if (xData && xData.length > 0) {
        const minTime = xData[0];
        const maxTime = xData[xData.length - 1];
        console.log('[ChartPanel] Auto-scaling to data range:', minTime, '-', maxTime);
        plotRef.current.setScale("x", { min: minTime, max: maxTime });
      }
    }
  }, [
    series,
    seriesData,
    follow,
    manualTimeRange,
    buildPlotDataAndMaps,
    buildAxesAndScales,
    buildSeriesOptions,
    rangeMinutes,
    onZoom,
  ]);

  useEffect(() => {
    if (!follow || !plotRef.current) {
      return;
    }
    const endSec = Date.now() / 1000;
    const startSec = endSec - rangeMinutes * 60;
    plotRef.current.setScale("x", { min: startSec, max: endSec });
  }, [follow, rangeMinutes]);

  useEffect(() => {
    if (!follow) {
      return;
    }
    const tick = () => {
      if (!plotRef.current) {
        return;
      }
      const endSec = Date.now() / 1000;
      const startSec = endSec - rangeMinutes * 60;
      plotRef.current.setScale("x", { min: startSec, max: endSec });
    };
    const timer = window.setInterval(tick, 1000);
    return () => window.clearInterval(timer);
  }, [follow, rangeMinutes]);

  return (
    <div className="flex flex-col h-full gap-2 border border-slate-800 rounded-md bg-slate-900/60 p-2">
      <div className="flex items-center justify-between">
        <div className="text-sm font-semibold text-slate-200">{title}</div>
        <div className="flex items-center gap-2">
          <Switch
            checked={showCommands}
            label="Commands"
            onChange={onToggleShowCommands}
          />
          <Button
            minimal
            small
            icon={IconNames.ZOOM_TO_FIT}
            text="Reset Zoom"
            onClick={() => {
              plotRef.current?.setScale("x", { min: null, max: null });
              onResetZoom();
            }}
          />
        </div>
      </div>
      <div className="flex-1 min-h-[260px] relative" ref={containerRef}></div>
      <div className="flex flex-col gap-1 text-xs text-slate-300">
        {series.length === 0 && <span>No series selected.</span>}
        {series.map((s) => (
          <div key={s.id} className="flex items-center gap-2">
            <span
              className="inline-block h-2 w-2 rounded-full"
              style={{ backgroundColor: s.color }}
            />
            <span className="flex-1 truncate">
              {s.tmivName}.{s.fieldName}
              {s.isRaw ? "@RAW" : ""}
            </span>
            <Button
              minimal
              small
              icon={IconNames.EDIT}
              onClick={() => onToggleSeriesMode(s.id)}
            />
            <Button
              minimal
              small
              icon={IconNames.CROSS}
              onClick={() => onRemoveSeries(s.id)}
            />
          </div>
        ))}
      </div>
      {(() => {
        // Collect all unique states from all series
        const stateSet = new Set<string>();
        const stateToValue = new Map<string, number>();

        series.forEach((s) => {
          const valueMap = valueMapsRef.current.get(s.id);
          if (valueMap) {
            valueMap.forEach((stateValue, stateLabel) => {
              stateSet.add(stateLabel);
              stateToValue.set(stateLabel, stateValue);
            });
          }
        });

        if (stateSet.size === 0) return null;

        const sortedStates = Array.from(stateSet).sort();

        return (
          <div className="mt-3 pt-3 border-t border-slate-700">
            <div className="text-xs font-semibold text-slate-400 mb-2">
              State Legend:
            </div>
            <div className="flex flex-col gap-1 text-xs text-slate-300">
              {sortedStates.map((stateLabel) => {
                const stateValue = stateToValue.get(stateLabel) ?? 0;
                const color = getStateColor(stateValue);
                return (
                  <div key={stateLabel} className="flex items-center gap-2">
                    <span
                      className="inline-block h-3 w-3 rounded"
                      style={{ backgroundColor: color }}
                    />
                    <span>{stateLabel}</span>
                  </div>
                );
              })}
            </div>
          </div>
        );
      })()}
    </div>
  );
};

export const ChartsView: React.FC = () => {
  // Try to get schema from WebSocket context, but make it optional
  let satelliteSchemaFromContext;
  try {
    const client = useClient();
    satelliteSchemaFromContext = client?.satelliteSchema;
  } catch (e) {
    // WebSocket context not available, will fetch from API instead
    satelliteSchemaFromContext = null;
  }

  const [satelliteSchema, setSatelliteSchema] = useState(satelliteSchemaFromContext);
  const [recorderEndpoint, setRecorderEndpointState] = useState(
    getRecorderEndpoint(),
  );
  const [sessionInfo, setSessionInfo] = useState<RecordingSession | null>(
    null,
  );
  const [recordings, setRecordings] = useState<RecordingListItem[]>([]);
  const [selectedSessionId, setSelectedSessionId] = useState<string>("active");

  // Debug: Log when recordings or selectedSessionId changes
  useEffect(() => {
    console.log("[DEBUG] recordings state updated:", recordings);
  }, [recordings]);

  useEffect(() => {
    console.log("[DEBUG] selectedSessionId changed to:", selectedSessionId);
  }, [selectedSessionId]);
  const [suffix, setSuffix] = useState<string>("");
  const [rangeMinutes, setRangeMinutes] = useState<number>(1);
  const [follow, setFollow] = useState<boolean>(true);
  const [timeRangeMs, setTimeRangeMs] = useState<{ min: number; max: number } | null>(null);

  // Time slider state for manual time range selection
  const [useTimeSlider, setUseTimeSlider] = useState<boolean>(false);
  const [sliderStartMs, setSliderStartMs] = useState<number>(0);
  const [sliderEndMs, setSliderEndMs] = useState<number>(0);

  // Track current zoom range (from drag zoom)
  const [zoomedRange, setZoomedRange] = useState<{ startMs: number; endMs: number } | null>(null);

  // Initialize Panel Series from localStorage
  const savedPanelState = getPanelSeriesState();
  const [panelASeries, setPanelASeries] = useState<SeriesConfig[]>(savedPanelState?.panelA ?? []);
  const [panelBSeries, setPanelBSeries] = useState<SeriesConfig[]>(savedPanelState?.panelB ?? []);
  const [seriesData, setSeriesData] = useState<SeriesDataMap>(new Map());
  const [commands, setCommands] = useState<CommandLogItem[]>([]);
  const [showCommandsA, setShowCommandsA] = useState<boolean>(true);
  const [showCommandsB, setShowCommandsB] = useState<boolean>(true);

  // Initialize Series Picker state from localStorage
  const savedPickerState = getSeriesPickerState();
  const [selectedTmiv, setSelectedTmiv] = useState<string>(savedPickerState?.selectedTmiv ?? "");
  const [selectedField, setSelectedField] = useState<string>(savedPickerState?.selectedField ?? "");
  const [selectedRaw, setSelectedRaw] = useState<boolean>(savedPickerState?.selectedRaw ?? false);
  const hasRestoredPickerFromStorage = useRef<boolean>(!!savedPickerState);

  const [tmivSearch, setTmivSearch] = useState<string>("");
  const [fieldSearch, setFieldSearch] = useState<string>("");
  const [showControls, setShowControls] = useState<boolean>(true);
  const valueMapsRefA = useRef<Map<string, Map<string, number>>>(new Map());
  const valueMapsRefB = useRef<Map<string, Map<string, number>>>(new Map());

  const tmivNames = useMemo(() => {
    if (!satelliteSchema) {
      return [];
    }
    const items: string[] = [];
    const channelNames = Object.keys(satelliteSchema.telemetryChannels || {});
    for (const [componentName, componentSchema] of Object.entries(
      satelliteSchema.telemetryComponents || {},
    )) {
      for (const telemetryName of Object.keys(componentSchema.telemetries || {})) {
        for (const channelName of channelNames) {
          items.push(`${channelName}.${componentName}.${telemetryName}`);
        }
      }
    }
    items.sort((a, b) => {
      const rtA = a.startsWith("RT.");
      const rtB = b.startsWith("RT.");
      if (rtA && !rtB) {
        return -1;
      }
      if (!rtA && rtB) {
        return 1;
      }
      return a.localeCompare(b);
    });
    const query = tmivSearch.trim().toLowerCase();
    if (!query) {
      return items;
    }
    return items.filter((name) => name.toLowerCase().includes(query));
  }, [satelliteSchema, tmivSearch]);

  const fieldNames = useMemo(() => {
    if (!selectedTmiv || !satelliteSchema) {
      return [];
    }
    const [, componentName, telemetryName] = selectedTmiv.split(".");
    const component = satelliteSchema.telemetryComponents?.[componentName];
    const telemetry = component?.telemetries?.[telemetryName];
    if (!telemetry) {
      return [];
    }
    const names = (telemetry.fields || []).map((f) => f.name).sort();
    const query = fieldSearch.trim().toLowerCase();
    if (!query) {
      return names;
    }
    return names.filter((name) => name.toLowerCase().includes(query));
  }, [satelliteSchema, selectedTmiv, fieldSearch]);

  const baseUrl = recorderEndpoint.replace(/\/$/, "");

  const refreshSessions = useCallback(async () => {
    try {
      const session = await fetchJson<CurrentSessionResponse>(
        `${baseUrl}/api/recording/session`,
      );
      setSessionInfo(session.session ?? null);
    } catch (e) {
      console.error(e);
      setSessionInfo(null);
    }
    try {
      const list = await fetchJson<RecordingListResponse>(
        `${baseUrl}/api/recordings/list`,
      );
      console.log("[DEBUG] Fetched recordings list:", list.recordings);
      setRecordings(list.recordings ?? []);
    } catch (e) {
      console.error("[DEBUG] Failed to fetch recordings:", e);
      setRecordings([]);
    }
  }, [baseUrl]);

  useEffect(() => {
    refreshSessions();
  }, [refreshSessions]);

  // Fetch schema from API if not available from WebSocket context
  useEffect(() => {
    const fetchSchema = async () => {
      if (satelliteSchema) {
        return; // Already have schema from WebSocket
      }

      try {
        const resp = await fetch(`${baseUrl}/api/schema`);
        if (resp.ok) {
          const schemaJson = await resp.json();
          setSatelliteSchema(schemaJson);
        }
      } catch (e) {
        console.error("Failed to fetch schema from API:", e);
      }
    };

    fetchSchema();
  }, [baseUrl, satelliteSchema]);

  // Fetch time range when session changes
  useEffect(() => {
    const fetchTimeRange = async () => {
      if (selectedSessionId === "active") {
        setTimeRangeMs(null);
        setFollow(true);
        return;
      }

      try {
        const params = buildQueryParams({ session_id: selectedSessionId });
        const resp = await fetchJson<TimeRangeResponse>(
          `${baseUrl}/api/telemetry/time_range?${params}`
        );

        if (resp.min_time_ms && resp.max_time_ms) {
          setTimeRangeMs({ min: resp.min_time_ms, max: resp.max_time_ms });
          setFollow(false);
        } else {
          setTimeRangeMs(null);
        }
      } catch (e) {
        console.error("Failed to fetch time range:", e);
        setTimeRangeMs(null);
      }
    };

    fetchTimeRange();
  }, [selectedSessionId, baseUrl]);

  const startRecording = useCallback(async () => {
    await fetchJson<StartRecordingResponse>(`${baseUrl}/api/recording/start`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ suffix }),
    });
    setSuffix("");
    await refreshSessions();
  }, [baseUrl, suffix, refreshSessions]);

  const stopRecording = useCallback(async () => {
    await fetchJson<StopRecordingResponse>(`${baseUrl}/api/recording/stop`, {
      method: "POST",
    });
    await refreshSessions();
  }, [baseUrl, refreshSessions]);

  const addSeries = useCallback(
    (panel: "A" | "B") => {
      if (!selectedTmiv || !selectedField) {
        return;
      }
      const id = buildSeriesId(selectedTmiv, selectedField, selectedRaw);
      const nextColorIndex =
        panel === "A" ? panelASeries.length : panelBSeries.length;
      const next = {
        id,
        tmivName: selectedTmiv,
        fieldName: selectedField,
        isRaw: selectedRaw,
        color: palette[nextColorIndex % palette.length],
      };
      if (panel === "A") {
        setPanelASeries((prev) => (prev.find((s) => s.id === id) ? prev : [...prev, next]));
      } else {
        setPanelBSeries((prev) => (prev.find((s) => s.id === id) ? prev : [...prev, next]));
      }
    },
    [panelASeries.length, panelBSeries.length, selectedField, selectedRaw, selectedTmiv],
  );

  const removeSeries = useCallback((panel: "A" | "B", id: string) => {
    if (panel === "A") {
      setPanelASeries((prev) => prev.filter((s) => s.id !== id));
      valueMapsRefA.current.delete(id);
    } else {
      setPanelBSeries((prev) => prev.filter((s) => s.id !== id));
      valueMapsRefB.current.delete(id);
    }
    setSeriesData((prev) => {
      const next = new Map(prev);
      next.delete(id);
      return next;
    });
  }, []);

  const toggleSeriesMode = useCallback((panel: "A" | "B", id: string) => {
    const valueMapsRef = panel === "A" ? valueMapsRefA : valueMapsRefB;
    const update = (prev: SeriesConfig[]) =>
      prev.map((s) => {
        if (s.id !== id) {
          return s;
        }
        const nextId = buildSeriesId(s.tmivName, s.fieldName, !s.isRaw);
        const existingMap = valueMapsRef.current.get(s.id);
        valueMapsRef.current.delete(s.id);
        if (existingMap) {
          valueMapsRef.current.set(nextId, existingMap);
        }
        return { ...s, isRaw: !s.isRaw, id: nextId };
      });
    if (panel === "A") {
      setPanelASeries(update);
    } else {
      setPanelBSeries(update);
    }
  }, []);

  const updateRecorderEndpoint = useCallback(
    (value: string) => {
      setRecorderEndpoint(value);
      setRecorderEndpointState(value);
    },
    [setRecorderEndpointState],
  );

  const activeSeries = useMemo(
    () => [...panelASeries, ...panelBSeries],
    [panelASeries, panelBSeries],
  );

  const isPlaybackMode = selectedSessionId !== "active";

  // Initialize slider values when time range is loaded
  useEffect(() => {
    if (timeRangeMs) {
      setSliderStartMs(timeRangeMs.min);
      setSliderEndMs(timeRangeMs.max);
    }
  }, [timeRangeMs]);

  const handleZoom = useCallback((startSec: number, endSec: number) => {
    setFollow(false);
    // Save zoom range for data re-fetching
    const startMs = Math.floor(startSec * 1000);
    const endMs = Math.floor(endSec * 1000);
    setZoomedRange({ startMs, endMs });
  }, []);

  const handleResetZoom = useCallback(() => {
    console.log('[handleResetZoom] isPlaybackMode:', isPlaybackMode, 'follow:', follow);
    // In playback mode, don't enable follow (which uses current time)
    // Instead, just clear the zoom range to show all data
    if (!isPlaybackMode) {
      setFollow(true);
    } else {
      setFollow(false);
    }
    setZoomedRange(null);
    setUseTimeSlider(false);
  }, [isPlaybackMode, follow]);

  useEffect(() => {
    if (activeSeries.length === 0) {
      setSeriesData(new Map());
      setCommands([]);
      return;
    }
    let timer: number | null = null;

    const refreshData = async () => {
      let endMs: number;
      let startMs: number;

      if (useTimeSlider && timeRangeMs) {
        // Use slider values for manual time selection
        startMs = sliderStartMs;
        endMs = sliderEndMs;
      } else if (zoomedRange) {
        // Use zoomed range from drag zoom
        startMs = zoomedRange.startMs;
        endMs = zoomedRange.endMs;
      } else if (timeRangeMs) {
        // Use database time range for past recordings
        startMs = timeRangeMs.min;
        endMs = timeRangeMs.max;
      } else {
        // Use current time for active session
        endMs = Date.now();
        startMs = endMs - rangeMinutes * 60 * 1000;
      }

      const nextSeriesData = new Map<string, TelemetrySample[]>();
      const sessionId =
        selectedSessionId === "active" ? undefined : selectedSessionId;

      // Calculate max_points based on time range for dynamic sampling
      const queryTimeRangeMs = endMs - startMs;
      const maxPoints = calculateMaxPoints(queryTimeRangeMs);
      console.log(`[DEBUG] Time range: ${(queryTimeRangeMs / 1000 / 60).toFixed(1)} min, max_points: ${maxPoints}`);

      for (const series of activeSeries) {
        const queryFieldName = convertFieldNameForQuery(series.fieldName, series.isRaw);
        console.log(`[DEBUG] Query params: tmiv_name=${series.tmivName}, field_name=${queryFieldName} (original: ${series.fieldName}), is_raw=${series.isRaw}`);
        const params = buildQueryParams({
          tmiv_name: series.tmivName,
          field_name: queryFieldName,
          is_raw: series.isRaw,
          start_ms: startMs,
          end_ms: endMs,
          max_points: maxPoints,
          session_id: sessionId,
        });
        console.log(`[DEBUG] Full URL: ${baseUrl}/api/telemetry/query?${params}`);
        try {
          const resp = await fetchJson<TelemetryQueryResponse>(
            `${baseUrl}/api/telemetry/query?${params}`,
          );
          console.log(`[DEBUG] Response for ${series.id}: ${resp.samples?.length ?? 0} samples`);
          nextSeriesData.set(series.id, resp.samples ?? []);
        } catch (e) {
          console.error(e);
        }
      }
      setSeriesData(nextSeriesData);
      try {
        const commandParams = buildQueryParams({
          start_ms: startMs,
          end_ms: endMs,
          max_points: 10000,  // Increased from 500 to show more commands
          session_id: sessionId,
        });
        const commandResp = await fetchJson<CommandQueryResponse>(
          `${baseUrl}/api/commands/query?${commandParams}`,
        );
        setCommands(commandResp.commands ?? []);
      } catch (e) {
        console.error(e);
      }
    };

    refreshData();
    timer = window.setInterval(refreshData, 2000);

    return () => {
      if (timer !== null) {
        window.clearInterval(timer);
      }
    };
  }, [
    activeSeries,
    baseUrl,
    follow,
    rangeMinutes,
    selectedSessionId,
    timeRangeMs,
    useTimeSlider,
    sliderStartMs,
    sliderEndMs,
    zoomedRange,
    // Note: tempSlider values are NOT in deps to avoid re-fetching while dragging
    // sliderStartMs/EndMs and zoomedRange trigger refresh
  ]);

  useEffect(() => {
    if (tmivNames.length === 0) {
      return;
    }
    // If we restored from storage and the value is valid, don't override it
    if (hasRestoredPickerFromStorage.current && selectedTmiv && tmivNames.includes(selectedTmiv)) {
      hasRestoredPickerFromStorage.current = false;
      return;
    }
    if (!selectedTmiv || !tmivNames.includes(selectedTmiv)) {
      setSelectedTmiv(tmivNames[0]);
    }
  }, [selectedTmiv, tmivNames]);

  useEffect(() => {
    if (!selectedTmiv || fieldNames.length === 0) {
      return;
    }
    // Only set to first item if there's no selection at all
    if (!selectedField) {
      setSelectedField(fieldNames[0]);
    } else if (!fieldNames.includes(selectedField)) {
      // Only override if the saved value is not in the list
      setSelectedField(fieldNames[0]);
    }
  }, [selectedTmiv, fieldNames, selectedField]);

  // Save Panel Series state to localStorage whenever it changes
  useEffect(() => {
    setPanelSeriesState(panelASeries, panelBSeries);
  }, [panelASeries, panelBSeries]);

  // Save Series Picker state to localStorage whenever it changes
  useEffect(() => {
    if (selectedTmiv && selectedField) {
      setSeriesPickerState(selectedTmiv, selectedField, selectedRaw);
    }
  }, [selectedTmiv, selectedField, selectedRaw]);

  return (
    <div className="h-full flex flex-col p-3 gap-3 text-slate-100 min-h-0">
      <div className="grid grid-cols-1 xl:grid-cols-[320px_1fr] gap-3 h-full min-h-0">
        <div className="bg-slate-900/80 border border-slate-800 rounded-md p-3 flex flex-col gap-3 overflow-y-auto min-h-0">
          <div
            className="text-sm font-semibold text-slate-200 flex items-center gap-2 cursor-pointer hover:text-slate-100"
            onClick={() => setShowControls(!showControls)}
          >
            <Icon icon={showControls ? IconNames.CHEVRON_DOWN : IconNames.CHEVRON_RIGHT} size={14} />
            <span>Controls</span>
          </div>
          {showControls && (
            <>
              <div className="text-sm font-semibold text-slate-200 flex items-center gap-2">
                <Icon icon={IconNames.EDIT} />
                Recording
                {isPlaybackMode && (
                  <span className="text-xs text-amber-400">(Playback Mode)</span>
                )}
              </div>
          <div className="flex flex-col gap-2">
            <InputGroup
              placeholder="Recorder endpoint"
              value={recorderEndpoint}
              onChange={(e) => updateRecorderEndpoint(e.target.value)}
              disabled={isPlaybackMode}
            />
            <div className="flex items-center gap-2">
              <InputGroup
                placeholder="Suffix"
                value={suffix}
                onChange={(e) => setSuffix(e.target.value)}
                disabled={isPlaybackMode}
              />
              <Button
                minimal
                icon={IconNames.PLAY}
                onClick={startRecording}
                disabled={isPlaybackMode}
              />
              <Button
                minimal
                icon={IconNames.STOP}
                onClick={stopRecording}
                disabled={isPlaybackMode}
              />
            </div>
            {sessionInfo && (
              <div className="text-xs text-slate-300">
                <div>Active: {sessionInfo.session_id}</div>
                <div>File: {sessionInfo.db_path}</div>
              </div>
            )}
            {isPlaybackMode && timeRangeMs && (
              <div className="text-xs text-slate-300">
                <div>Time range: {new Date(timeRangeMs.min).toLocaleString()} - {new Date(timeRangeMs.max).toLocaleString()}</div>
              </div>
            )}
          </div>
          <div className="text-sm font-semibold text-slate-200">Recording Files</div>
          <div className="flex flex-col gap-2">
            <select
              className={Classes.HTML_SELECT}
              value={selectedSessionId}
              onChange={(e) => {
                console.log("[DEBUG] Dropdown onChange called, new value:", e.target.value);
                console.log("[DEBUG] Current recordings:", recordings);
                setSelectedSessionId(e.target.value);
              }}
            >
              <option value="active">Active session</option>
              {recordings.map((item) => (
                <option key={item.session_id} value={item.session_id}>
                  {item.session_id}
                  {item.suffix ? `_${item.suffix}` : ""}
                </option>
              ))}
            </select>
          </div>
          <div className="text-sm font-semibold text-slate-200">Debug</div>
          <div className="flex gap-2">
            <Button
              minimal
              small
              icon={IconNames.DOWNLOAD}
              text="Download Logs"
              onClick={downloadDebugLogs}
            />
            <Button
              minimal
              small
              icon={IconNames.TRASH}
              text="Clear"
              onClick={clearDebugLogs}
            />
          </div>
          <div className="text-sm font-semibold text-slate-200">Series Picker</div>
          <div className="flex flex-col gap-2">
            <InputGroup
              placeholder="Search telemetry"
              value={tmivSearch}
              onChange={(e) => setTmivSearch(e.target.value)}
            />
            <select
              className={Classes.HTML_SELECT}
              value={selectedTmiv}
              onChange={(e) => setSelectedTmiv(e.target.value)}
            >
              {tmivNames.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>
            <InputGroup
              placeholder="Search field"
              value={fieldSearch}
              onChange={(e) => setFieldSearch(e.target.value)}
            />
            <select
              className={Classes.HTML_SELECT}
              value={selectedField}
              onChange={(e) => setSelectedField(e.target.value)}
            >
              {fieldNames.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </select>
            <Switch
              checked={selectedRaw}
              label="RAW (@RAW)"
              onChange={() => setSelectedRaw((prev) => !prev)}
            />
            <div className="flex items-center gap-2">
              <Button
                icon={IconNames.ADD}
                onClick={() => addSeries("A")}
                disabled={!selectedTmiv || !selectedField}
              >
                Add to Panel A
              </Button>
              <Button
                icon={IconNames.ADD}
                onClick={() => addSeries("B")}
                disabled={!selectedTmiv || !selectedField}
              >
                Add to Panel B
              </Button>
            </div>
          </div>
          <div className="text-sm font-semibold text-slate-200">Time Range</div>
          <div className="flex flex-col gap-2">
            <InputGroup
              type="number"
              min={1}
              value={String(rangeMinutes)}
              onChange={(e) => setRangeMinutes(Number(e.target.value))}
              rightElement={<span className="pr-2 text-xs">min</span>}
            />
            <Switch
              checked={follow}
              label="Follow latest"
              onChange={() => setFollow((prev) => !prev)}
            />
            {isPlaybackMode && timeRangeMs && (
              <>
                <Switch
                  checked={useTimeSlider}
                  label="Manual time selection"
                  onChange={() => setUseTimeSlider((prev) => !prev)}
                />
                {useTimeSlider && (
                  <div className="flex flex-col gap-2 mt-2 p-2 bg-slate-800 rounded">
                    <div className="text-xs text-slate-400">Start time</div>
                    <input
                      type="range"
                      min={timeRangeMs.min}
                      max={timeRangeMs.max}
                      value={sliderStartMs}
                      onChange={(e) => setSliderStartMs(Number(e.target.value))}
                      className="w-full"
                    />
                    <div className="text-xs text-slate-300">
                      {new Date(sliderStartMs).toLocaleString()}
                    </div>
                    <div className="text-xs text-slate-400 mt-2">End time</div>
                    <input
                      type="range"
                      min={timeRangeMs.min}
                      max={timeRangeMs.max}
                      value={sliderEndMs}
                      onChange={(e) => setSliderEndMs(Number(e.target.value))}
                      className="w-full"
                    />
                    <div className="text-xs text-slate-300">
                      {new Date(sliderEndMs).toLocaleString()}
                    </div>
                    <button
                      onClick={() => {
                        setSliderStartMs(timeRangeMs.min);
                        setSliderEndMs(timeRangeMs.max);
                      }}
                      className="mt-2 px-2 py-1 text-xs bg-slate-700 hover:bg-slate-600 rounded"
                    >
                      Reset to full range
                    </button>
                  </div>
                )}
              </>
            )}
          </div>
            </>
          )}
        </div>
        <div className="flex flex-col gap-3 min-h-0 overflow-y-auto pb-4">
          <ChartPanel
            title="Panel A"
            series={panelASeries}
            seriesData={seriesData}
            commands={commands}
            showCommands={showCommandsA}
            follow={follow}
            rangeMinutes={rangeMinutes}
            manualTimeRange={
              useTimeSlider
                ? { startMs: sliderStartMs, endMs: sliderEndMs }
                : zoomedRange
                ? { startMs: zoomedRange.startMs, endMs: zoomedRange.endMs }
                : null
            }
            onToggleShowCommands={() => setShowCommandsA((prev) => !prev)}
            onRemoveSeries={(id) => removeSeries("A", id)}
            onToggleSeriesMode={(id) => toggleSeriesMode("A", id)}
            onZoom={handleZoom}
            onResetZoom={handleResetZoom}
            valueMapsRef={valueMapsRefA}
          />
          <ChartPanel
            title="Panel B"
            series={panelBSeries}
            seriesData={seriesData}
            commands={commands}
            showCommands={showCommandsB}
            follow={follow}
            rangeMinutes={rangeMinutes}
            manualTimeRange={
              useTimeSlider
                ? { startMs: sliderStartMs, endMs: sliderEndMs }
                : zoomedRange
                ? { startMs: zoomedRange.startMs, endMs: zoomedRange.endMs }
                : null
            }
            onToggleShowCommands={() => setShowCommandsB((prev) => !prev)}
            onRemoveSeries={(id) => removeSeries("B", id)}
            onToggleSeriesMode={(id) => toggleSeriesMode("B", id)}
            onZoom={handleZoom}
            onResetZoom={handleResetZoom}
            valueMapsRef={valueMapsRefB}
          />
        </div>
      </div>
    </div>
  );
};

type CurrentSessionResponse = {
  session?: RecordingSession;
};

type StartRecordingResponse = {
  session: RecordingSession;
};

type StopRecordingResponse = {
  session?: RecordingSession;
};

type RecordingListResponse = {
  recordings: RecordingListItem[];
};

type TelemetryQueryResponse = {
  samples: TelemetrySample[];
};

type CommandQueryResponse = {
  commands: CommandLogItem[];
};

type TimeRangeResponse = {
  min_time_ms: number | null;
  max_time_ms: number | null;
};
