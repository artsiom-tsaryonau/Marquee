export function hours(min: number) {
  if (min > 0 && min < 1) return `${Math.max(1, Math.round(min * 60))} s`;
  if (min < 60) return `${Math.round(min)} min`;
  const h = min / 60;
  return `${h < 10 ? h.toFixed(1) : Math.round(h)} h`;
}

export function describeScan(report: {
  added: number;
  updated: number;
  skipped_unsupported: number;
  skipped_by_kind: { kind: string; count: number }[];
  unresolved_serial: number;
  missing: number;
  warnings?: string[];
}) {
  const skipped = report.skipped_by_kind
    .map((s) => `${s.count} ${s.kind.replace('ps2_', '').toUpperCase()}`)
    .join(', ');
  const parts = [
    `Imported ${report.added} ISO file${report.added === 1 ? '' : 's'}`,
    report.updated ? `${report.updated} unchanged` : '',
    report.skipped_unsupported
      ? `skipped ${report.skipped_unsupported} unsupported file${report.skipped_unsupported === 1 ? '' : 's'}${skipped ? ` (${skipped})` : ''} because those formats are not supported yet`
      : '',
    report.unresolved_serial ? `${report.unresolved_serial} without a serial` : '',
    report.missing ? `${report.missing} missing` : '',
    report.warnings?.length ? `${report.warnings.length} file warning(s): ${report.warnings[0]}` : '',
  ].filter(Boolean);
  return parts.join(' · ');
}

export function ago(iso: string) {
  const days = Math.floor((Date.now() - new Date(iso).getTime()) / 86400000);
  if (days <= 0) return 'today';
  if (days === 1) return 'yesterday';
  if (days < 30) return `${days} days ago`;
  const months = Math.floor(days / 30);
  return months === 1 ? 'a month ago' : `${months} months ago`;
}

/** Subsequence fuzzy match; higher is better, -1 is no match. */
export function fuzzy(query: string, text: string) {
  const q = query.toLowerCase().replace(/\s+/g, '');
  const t = text.toLowerCase();
  if (!q) return 0;
  let ti = 0;
  let score = 0;
  let streak = 0;
  for (const c of q) {
    const found = t.indexOf(c, ti);
    if (found < 0) return -1;
    streak = found === ti ? streak + 1 : 0;
    score += 1 + streak * 2 + (found === 0 || t[found - 1] === ' ' ? 3 : 0);
    ti = found + 1;
  }
  return score - t.length * 0.01;
}

export function bytes(n: number) {
  if (n < 1024) return `${n} B`;
  const units = ['KB', 'MB', 'GB', 'TB'];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

/** One plain line about a transfer, e.g. "Sharing with your other devices · 2.1 GB uploaded". */
export function transferLine(t: { state: string; kind: string; downloaded: number; total: number; uploaded: number; error: string | null }) {
  switch (t.state) {
    case 'preparing': return 'Preparing to share. Large games take a while to check.';
    case 'starting': return 'Starting…';
    case 'checking': return 'Checking files…';
    case 'downloading': return `Downloading · ${bytes(t.downloaded)} of ${bytes(t.total)}${t.total ? ` (${Math.floor((t.downloaded / t.total) * 100)}%)` : ''}`;
    case 'paused': return 'Paused';
    case 'error': return t.error ?? 'Something went wrong';
    default: return t.kind === 'get' ? `Done · sharing it onward · ${bytes(t.uploaded)} uploaded` : `Sharing with your other devices · ${bytes(t.uploaded)} uploaded`;
  }
}
