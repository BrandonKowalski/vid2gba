type Which = 'start' | 'end';

const el = (id: string) => document.getElementById(id) as HTMLElement;

export function placeTrim(start: number, end: number, duration: number) {
  const pct = (t: number) => (duration > 0 ? Math.min(100, Math.max(0, (t / duration) * 100)) : 0);
  el('trim-start').style.left = `${pct(start)}%`;
  el('trim-end').style.left = `${pct(end)}%`;
  el('trim-range').style.left = `${pct(start)}%`;
  el('trim-range').style.right = `${100 - pct(end)}%`;
  for (const [id, v] of [['trim-start', start], ['trim-end', end]] as const) {
    el(id).setAttribute('aria-valuemin', '0');
    el(id).setAttribute('aria-valuemax', duration.toFixed(1));
    el(id).setAttribute('aria-valuenow', v.toFixed(1));
  }
}

export function mountTrim(duration: () => number, value: (w: Which) => number, change: (w: Which, t: number) => void) {
  const track = el('trim');
  for (const which of ['start', 'end'] as const) {
    const handle = el(`trim-${which}`);
    let dragging = false;
    const at = (x: number) => {
      const r = track.getBoundingClientRect();
      return Math.min(1, Math.max(0, (x - r.left) / r.width)) * duration();
    };
    handle.addEventListener('pointerdown', (ev) => {
      ev.preventDefault();
      dragging = true;
      handle.classList.add('dragging');
      if (ev.isTrusted) handle.setPointerCapture(ev.pointerId);
    });
    handle.addEventListener('pointermove', (ev) => {
      if (dragging) change(which, at(ev.clientX));
    });
    for (const t of ['pointerup', 'pointercancel', 'lostpointercapture']) {
      handle.addEventListener(t, () => {
        dragging = false;
        handle.classList.remove('dragging');
      });
    }
    handle.addEventListener('keydown', (ev) => {
      const size = ev.shiftKey ? 1 : 0.1;
      const delta = ev.key === 'ArrowRight' || ev.key === 'ArrowUp' ? size : ev.key === 'ArrowLeft' || ev.key === 'ArrowDown' ? -size : 0;
      if (!delta) return;
      ev.preventDefault();
      change(which, Math.min(duration(), Math.max(0, value(which) + delta)));
    });
  }
}
