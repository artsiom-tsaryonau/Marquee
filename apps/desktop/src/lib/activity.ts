/** Poll only while this window is visible and focused. A slow request never
 * overlaps the next tick; resume refreshes immediately, and disposal is final. */
export function pollWhileActive(refresh: () => Promise<void>, interval: () => number) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let stopped = false;
  let inFlight = false;
  const active = () => !document.hidden && document.hasFocus();
  const clear = () => { clearTimeout(timer); timer = undefined; };
  const tick = async () => {
    clear();
    if (stopped || inFlight || !active()) return;
    inFlight = true;
    try { await refresh(); }
    finally {
      inFlight = false;
      if (!stopped && active()) timer = setTimeout(() => void tick(), interval());
    }
  };
  const changed = () => { clear(); if (!stopped && active()) void tick(); };
  document.addEventListener('visibilitychange', changed);
  window.addEventListener('focus', changed);
  window.addEventListener('blur', changed);
  void tick();
  return () => {
    stopped = true;
    clear();
    document.removeEventListener('visibilitychange', changed);
    window.removeEventListener('focus', changed);
    window.removeEventListener('blur', changed);
  };
}
