export function formatTimestamp(ms?: string | number): string {
  if (!ms) return '-';
  const n = typeof ms === 'string' ? parseInt(ms, 10) : ms;
  if (Number.isNaN(n)) return '-';
  return new Date(n).toLocaleString('zh-CN');
}

export function formatDuration(ms?: number): string {
  if (ms == null) return '-';
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
  const m = Math.floor(ms / 60000);
  const s = ((ms % 60000) / 1000).toFixed(0);
  return `${m}m${s}s`;
}

export function calcDuration(start?: string, end?: string): string {
  if (!start || !end) return '-';
  const s = parseInt(start, 10);
  const e = parseInt(end, 10);
  if (Number.isNaN(s) || Number.isNaN(e)) return '-';
  return formatDuration(e - s);
}
