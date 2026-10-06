const MAX_W = 232;
const SIZES = [12, 11, 10, 9];

function wrap(lines: string[], size: number, measure: (s: string, size: number) => number): string[] {
  const out: string[] = [];
  for (const line of lines) {
    let cur = '';
    for (const word of line.split(/\s+/).filter(Boolean)) {
      const candidate = cur ? `${cur} ${word}` : word;
      if (measure(candidate, size) <= MAX_W) {
        cur = candidate;
        continue;
      }
      if (cur) {
        out.push(cur);
        cur = '';
      }
      for (const ch of Array.from(word)) {
        if (cur && measure(cur + ch, size) > MAX_W) {
          out.push(cur);
          cur = ch;
        } else {
          cur += ch;
        }
      }
    }
    if (cur) out.push(cur);
  }
  return out;
}

export function renderCue(text: string): { alpha: Uint8Array; w: number; h: number } {
  const canvas = new OffscreenCanvas(256, 32);
  const ctx = canvas.getContext('2d', { willReadFrequently: true })!;
  const measure = (s: string, size: number) => {
    ctx.font = `${size}px sans-serif`;
    return ctx.measureText(s).width;
  };
  const lines = text.split('\n');
  let size = SIZES[SIZES.length - 1];
  let fitted: string[] | null = null;
  for (const s of SIZES) {
    const w = wrap(lines, s, measure);
    if (w.length <= 2) {
      size = s;
      fitted = w;
      break;
    }
  }
  if (!fitted) {
    fitted = wrap(lines, size, measure).slice(0, 2);
    let last = fitted[1];
    while (last && measure(`${last.trimEnd()}…`, size) > MAX_W) last = Array.from(last).slice(0, -1).join('');
    fitted[1] = `${last.trimEnd()}…`;
  }
  ctx.font = `${size}px sans-serif`;
  const widths = fitted.map((l) => Math.ceil(ctx.measureText(l).width));
  const lineH = Math.ceil(size * 1.2);
  const w = Math.min(MAX_W + 2, Math.max(2, Math.max(...widths) + 2));
  const h = Math.min(32, lineH * fitted.length + 2);
  canvas.width = w;
  canvas.height = h;
  ctx.font = `${size}px sans-serif`;
  ctx.fillStyle = '#fff';
  ctx.textBaseline = 'alphabetic';
  const ascent = Math.ceil(ctx.measureText('Hg').fontBoundingBoxAscent || size * 0.8);
  fitted.forEach((line, i) => {
    ctx.fillText(line, 1 + Math.max(0, Math.floor((w - 2 - widths[i]) / 2)), 1 + ascent + i * lineH);
  });
  const data = ctx.getImageData(0, 0, w, h).data;
  const alpha = new Uint8Array(w * h);
  for (let i = 0; i < w * h; i++) alpha[i] = data[i * 4 + 3];
  return { alpha, w, h };
}
