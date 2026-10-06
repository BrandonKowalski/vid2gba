import { $, busy, entryTrim, output, refresh, state, subsUsable } from './main';
import type { ConvertRequest, WorkerMessage } from './protocol';

let worker: Worker | null = null;
let pendingTitle = 'video';
export const onDone: { handler: (rom: Uint8Array, realGba: boolean) => void } = { handler: () => {} };

function stop() {
  worker?.terminate();
  worker = null;
  busy.converting = false;
  $('progress').hidden = true;
  refresh();
}

function fail(text: string) {
  stop();
  const el = $('error');
  el.textContent = text;
  el.hidden = false;
}

export function safeName(title: string): string {
  const name = title.replace(/[^A-Za-z0-9-]+/g, '_').replace(/^_+|_+$/g, '');
  return `${name || 'video'}.gba`;
}

export function startConvert() {
  if (busy.converting || state.entries.length === 0) return;
  if (state.entries.some((e) => typeof entryTrim(e) === 'string')) return;
  const entries = state.entries.map((e) => {
    const t = entryTrim(e) as { start: number; end: number };
    return {
      file: e.file,
      start: t.start,
      end: t.end,
      width: e.info.width,
      height: e.info.height,
      audio: e.info.audio,
      title: e.title.trim() || 'video',
      subtitles: subsUsable(e) ? e.subsText : null,
    };
  });
  const cartTitle = entries.length > 1 ? $<HTMLInputElement>('cart-title').value.trim() || 'vid2gba' : entries[0].title;
  const fps = $<HTMLSelectElement>('fps').value;
  const req: ConvertRequest = {
    entries,
    cartTitle,
    fps: fps ? Number(fps) : null,
    halfRes: $<HTMLInputElement>('half').checked,
    realGba: (document.querySelector('input[name="target"]:checked') as HTMLInputElement).value === 'gba',
  };
  pendingTitle = cartTitle;
  busy.converting = true;
  $('error').hidden = true;
  $<HTMLProgressElement>('bar').value = 0;
  $('progress-label').textContent = 'Starting…';
  $('progress').hidden = false;
  refresh();
  worker = new Worker(new URL('./convert.worker.ts', import.meta.url), { type: 'module' });
  worker.onerror = (e) => fail(`Conversion failed: ${e.message}`);
  worker.onmessage = (e: MessageEvent<WorkerMessage>) => {
    const m = e.data;
    if (m.type === 'progress') {
      $<HTMLProgressElement>('bar').value = m.fraction;
      $('progress-label').textContent = m.label;
    } else if (m.type === 'error') {
      fail(m.message);
    } else {
      stop();
      output.rom = m.rom;
      output.title = pendingTitle;
      $('summary').textContent = m.summary;
      $('gba-note').hidden = !m.realGba;
      onDone.handler(m.rom, m.realGba);
    }
  };
  worker.postMessage(req);
}

export function cancel() {
  stop();
}

export function download() {
  if (!output.rom) return;
  const url = URL.createObjectURL(new Blob([output.rom as BlobPart], { type: 'application/octet-stream' }));
  const a = document.createElement('a');
  a.href = url;
  a.download = safeName(output.title);
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}
