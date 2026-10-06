import { ALL_FORMATS, AudioSampleSink, BlobSource, CanvasSink, Input } from 'mediabunny';
import init, {
  AudioEncoder,
  CartBuilder,
  type JsLayout,
  type JsStep,
  Splitter,
  Subtitles,
  audio_too_big_message,
  candidates,
  fit_layout,
  max_rom_size,
  too_big_message,
} from './wasm-pkg/vid2gba.js';
import { Pool } from './pool';
import { renderCue } from './subtext';
import type { ConvertRequest, EntryRequest, WorkerMessage } from './protocol';

const RATE = 13379;

function post(m: WorkerMessage, transfer: Transferable[] = []) {
  (self as unknown as Worker).postMessage(m, transfer);
}

async function encodeAudio(input: Input, req: EntryRequest): Promise<{ data: Uint8Array; samples: number }> {
  const track = req.audio ? await input.getPrimaryAudioTrack() : null;
  if (!track) {
    const enc = new AudioEncoder(RATE);
    enc.push_silence(Math.round((req.end - req.start) * RATE));
    const data = enc.finish();
    return { data, samples: enc.samples() };
  }
  let enc: AudioEncoder | null = null;
  let written = 0;
  for await (const sample of new AudioSampleSink(track).samples(req.start, req.end)) {
    try {
      const rate = sample.sampleRate;
      enc ??= new AudioEncoder(rate);
      const at = Math.round((Math.max(sample.timestamp, req.start) - req.start) * rate);
      if (at > written) {
        enc.push(new Float32Array(at - written));
        written = at;
      }
      const n = sample.numberOfFrames;
      const ch = sample.numberOfChannels;
      const from = Math.max(0, Math.round((req.start - sample.timestamp) * rate));
      const to = Math.min(n, Math.round((req.end - sample.timestamp) * rate));
      if (to <= from) continue;
      const mono = new Float32Array(to - from);
      const plane = new Float32Array(n);
      for (let c = 0; c < ch; c++) {
        sample.copyTo(plane, { format: 'f32-planar', planeIndex: c });
        for (let i = from; i < to; i++) mono[i - from] += plane[i] / ch;
      }
      enc.push(mono);
      written += mono.length;
    } finally {
      sample.close();
    }
  }
  enc ??= new AudioEncoder(RATE);
  const data = enc.finish();
  return { data, samples: enc.samples() };
}

function downscale(
  src: OffscreenCanvas | HTMLCanvasElement,
  ctx: OffscreenCanvasRenderingContext2D,
  w: number,
  h: number,
  scratch: OffscreenCanvas[],
) {
  let cur: OffscreenCanvas | HTMLCanvasElement = src;
  for (let i = 0; cur.width > w * 2 && cur.height > h * 2; i++) {
    const nw = Math.max(w, Math.ceil(cur.width / 2));
    const nh = Math.max(h, Math.ceil(cur.height / 2));
    const c = (scratch[i] ??= new OffscreenCanvas(nw, nh));
    if (c.width !== nw || c.height !== nh) {
      c.width = nw;
      c.height = nh;
    }
    const x = c.getContext('2d')!;
    x.imageSmoothingEnabled = true;
    x.imageSmoothingQuality = ctx.imageSmoothingQuality;
    x.drawImage(cur, 0, 0, nw, nh);
    cur = c;
  }
  ctx.drawImage(cur, 0, 0, w, h);
}

async function thumbnail(input: Input, entry: EntryRequest): Promise<Uint8Array> {
  const canvas = new OffscreenCanvas(64, 40);
  const ctx = canvas.getContext('2d', { willReadFrequently: true })!;
  ctx.fillStyle = '#000';
  ctx.fillRect(0, 0, 64, 40);
  const track = await input.getPrimaryVideoTrack();
  if (track) {
    const sink = new CanvasSink(track, { poolSize: 1 });
    const at = entry.start + (entry.end - entry.start) * 0.1;
    for await (const wrapped of sink.canvasesAtTimestamps([at])) {
      if (!wrapped) continue;
      const scale = Math.min(64 / wrapped.canvas.width, 40 / wrapped.canvas.height);
      const w = Math.round(wrapped.canvas.width * scale);
      const h = Math.round(wrapped.canvas.height * scale);
      ctx.imageSmoothingQuality = 'high';
      ctx.drawImage(wrapped.canvas, Math.floor((64 - w) / 2), Math.floor((40 - h) / 2), w, h);
    }
  }
  const d = ctx.getImageData(0, 0, 64, 40).data;
  return new Uint8Array(d.buffer, d.byteOffset, d.byteLength);
}

async function encodeStep(
  input: Input,
  req: EntryRequest,
  realGba: boolean,
  step: JsStep,
  layout: JsLayout,
  cart: CartBuilder,
  pool: Pool,
  onFrame: (n: number) => void,
): Promise<boolean> {
  const track = await input.getPrimaryVideoTrack();
  if (!track) throw new Error('No video track.');
  const sink = new CanvasSink(track, { poolSize: 2 });
  const canvas = new OffscreenCanvas(layout.w, layout.h);
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) throw new Error('No 2D canvas.');
  ctx.imageSmoothingEnabled = true;
  ctx.imageSmoothingQuality = 'high';
  const splitter = new Splitter(step.fps, layout.w, layout.h, realGba);
  const pending: Promise<Uint8Array>[] = [];
  const limit = pool.size * 2;
  let over = false;
  const drain = async (keep: number) => {
    while (!over && pending.length > keep) {
      if (!cart.accept(await pending.shift()!)) over = true;
    }
  };
  const submit = async (gop: Uint8Array) => {
    pending.push(pool.encode({ gop, w: layout.w, h: layout.h, codebook: step.codebook, skip: step.skip }));
    await drain(limit - 1);
  };
  function* times() {
    for (let i = 0; ; i++) {
      const t = req.start + i / step.fps;
      if (t >= req.end) return;
      yield t;
    }
  }
  const scratch: OffscreenCanvas[] = [];
  let count = 0;
  let leading = 0;
  let last: Uint8Array | null = null;
  const push = async (rgba: Uint8Array) => {
    const gop = splitter.push(rgba);
    onFrame(++count);
    if (gop) await submit(gop);
  };
  for await (const wrapped of sink.canvasesAtTimestamps(times())) {
    if (over) break;
    if (!wrapped) {
      if (last) await push(last);
      else leading++;
      continue;
    }
    downscale(wrapped.canvas, ctx, layout.w, layout.h, scratch);
    const data = ctx.getImageData(0, 0, layout.w, layout.h).data;
    last = new Uint8Array(data.buffer, data.byteOffset, data.byteLength);
    for (; leading > 0 && !over; leading--) await push(last);
    if (!over) await push(last);
  }
  if (!over) {
    const last = splitter.finish();
    if (last) await submit(last);
    await drain(0);
  }
  pending.length = 0;
  splitter.free();
  if (over) return false;
  if (count === 0) throw new Error('No video frames were decoded.');
  cart.end_video(layout, step.fps);
  return true;
}

self.onmessage = async (e: MessageEvent<ConvertRequest>) => {
  const req = e.data;
  const n = req.entries.length;
  let pool: Pool | null = null;
  try {
    await init();
    const inputs = req.entries.map((x) => new Input({ source: new BlobSource(x.file), formats: ALL_FORMATS }));
    const cart = new CartBuilder();
    let audioBytes = 0;
    let anySubs = false;
    for (let i = 0; i < n; i++) {
      const entry = req.entries[i];
      post({ type: 'progress', fraction: i / n, label: n > 1 ? `Decoding audio · video ${i + 1} of ${n}…` : 'Decoding audio…' });
      const audio = await encodeAudio(inputs[i], entry);
      const subs = entry.subtitles ? new Subtitles(entry.subtitles, entry.start, entry.end) : Subtitles.empty();
      for (let k = 0; k < subs.count(); k++) {
        const { alpha, w, h } = renderCue(subs.text(k));
        subs.set_image(k, alpha, w, h);
      }
      cart.add_video(entry.width, entry.height, audio.data, audio.samples, subs);
      if (n > 1) cart.add_menu_entry(entry.title, entry.end - entry.start, await thumbnail(inputs[i], entry));
      audioBytes += audio.data.length;
      anySubs ||= subs.count() > 0;
      subs.free();
    }
    const max = max_rom_size();
    const secs = req.entries.reduce((t, x) => t + (x.end - x.start), 0);
    const fixed = cart.fixed_size(req.cartTitle);
    if (fixed > max) {
      post({ type: 'error', message: audio_too_big_message(secs, max, fixed, audioBytes, anySubs) });
      return;
    }
    pool = new Pool(Math.max(1, Math.min(navigator.hardwareConcurrency || 4, 8)));
    const steps = candidates(req.fps ?? undefined, req.halfRes);
    let best = 0;
    for (let i = 0; i < steps.length; i++) {
      const step = steps[i];
      cart.begin_step(max - fixed);
      let fit = true;
      let last: JsLayout | null = null;
      for (let v = 0; v < n && fit; v++) {
        const entry = req.entries[v];
        const layout = fit_layout(entry.width, entry.height, step.half_res);
        last = layout;
        const expected = Math.max(1, Math.ceil((entry.end - entry.start) * step.fps));
        const label = `Step ${i + 1} of ${steps.length}${n > 1 ? ` · video ${v + 1} of ${n}` : ''} · ${layout.w}×${layout.h} @ ${step.fps} fps`;
        fit = await encodeStep(inputs[v], entry, req.realGba, step, layout, cart, pool, (k) => {
          const f = Math.min(1, k / expected);
          post({ type: 'progress', fraction: (v + f) / n, label: `${label} · ${Math.round(f * 100)}%` });
        });
      }
      if (fit && last) {
        const rom = cart.build(req.cartTitle);
        const shape = n === 1 ? `${last.w}×${last.h}${last.half_res ? ' (half-res)' : ''} @ ${step.fps} fps` : `${n} videos @ ${step.fps} fps`;
        const summary = `${(rom.length / 1048576).toFixed(1)} MiB · ${shape}`;
        post({ type: 'done', rom, summary, realGba: req.realGba }, [rom.buffer]);
        return;
      }
      best = Math.max(best, cart.frames_fit() / step.fps);
    }
    post({ type: 'error', message: too_big_message(secs, max, best) });
  } catch (err) {
    post({ type: 'error', message: `Conversion failed: ${err instanceof Error ? err.message : String(err)}` });
  } finally {
    pool?.terminate();
  }
};
