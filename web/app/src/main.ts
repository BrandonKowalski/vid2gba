import '@fontsource/press-start-2p/400.css';
import '@fontsource/ibm-plex-mono/400.css';
import '@fontsource/ibm-plex-mono/600.css';
import init, { Subtitles, parse_time, validate_range } from './wasm-pkg/vid2gba.js';
import { inspect, type MediaInfo } from './media';
import { missingFeatures } from './support';
import { Preview } from './preview';
import { mountGba } from './gba';
import { mountTrim, placeTrim } from './trim';
import { thumbnail } from './thumb';
import { go, reachable, render, step, type Step } from './wizard';

export const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

export interface Entry {
  file: File;
  info: MediaInfo;
  title: string;
  start: string;
  end: string;
  subsText: string | null;
  subsError: string | null;
  subsName: string | null;
  thumb: string | null;
}

export const state: { entries: Entry[]; selected: number } = { entries: [], selected: -1 };

export const output: { rom: Uint8Array | null; title: string } = { rom: null, title: 'video' };

function current(): Entry | null {
  return state.entries[state.selected] ?? null;
}

export function entryTrim(e: Entry): { start: number; end: number } | string {
  try {
    const start = e.start.trim() ? parse_time(e.start) : undefined;
    const end = e.end.trim() ? parse_time(e.end) : undefined;
    validate_range(start, end, e.info.duration);
    return { start: start ?? 0, end: Math.min(end ?? e.info.duration, e.info.duration) };
  } catch (err) {
    return (err as Error).message;
  }
}

export function subsUsable(e: Entry): boolean {
  return e.subsText !== null && e.subsError === null;
}

function bounds(e: Entry): { start: number; end: number } {
  const t = entryTrim(e);
  return typeof t === 'string' ? { start: 0, end: e.info.duration } : t;
}

function setTrim(which: 'start' | 'end', t: number) {
  const e = current();
  if (!e) return;
  edited();
  const b = bounds(e);
  const d = e.info.duration;
  const v = which === 'start' ? Math.max(0, Math.min(t, b.end - 0.1)) : Math.min(d, Math.max(t, b.start + 0.1));
  const whole = which === 'start' ? v <= 0.05 : v >= d - 0.05;
  e[which] = whole ? '' : formatTime(v);
  $<HTMLInputElement>(which).value = e[which];
  $<HTMLVideoElement>('video').currentTime = v;
  refresh();
}

export function formatTime(t: number): string {
  const r = Math.round(t * 10) / 10;
  const m = Math.floor(r / 60);
  const s = r - m * 60;
  return `${m}:${s.toFixed(1).padStart(4, '0')}`;
}

function message(id: string, text: string | null) {
  const el = $(id);
  el.textContent = text ?? '';
  el.hidden = text === null;
}

export const busy = { converting: false };

function refreshSubs(e: Entry) {
  const info = $('subs-info');
  $('subs-clear').hidden = e.subsText === null && e.subsError === null;
  if (e.subsText !== null && e.subsError === null) {
    try {
      new Subtitles(e.subsText, 0, e.info.duration).free();
    } catch (err) {
      e.subsError = (err as Error).message;
    }
  }
  if (e.subsError !== null) {
    info.hidden = false;
    info.className = 'error';
    info.textContent = `Subtitles: ${e.subsError}`;
    return;
  }
  if (e.subsText === null) {
    info.hidden = true;
    return;
  }
  info.hidden = false;
  info.className = 'notice';
  info.textContent = e.subsName ?? 'Subtitles added';
}

function button(className: string, text: string, label: string, onClick: () => void): HTMLButtonElement {
  const b = Object.assign(document.createElement('button'), { className, type: 'button', textContent: text });
  b.setAttribute('aria-label', label);
  b.addEventListener('click', onClick);
  return b;
}

function renderList() {
  $('videos').replaceChildren(
    ...state.entries.map((e, i) => {
      const li = document.createElement('li');
      li.dataset.index = String(i);
      const thumb = document.createElement(e.thumb ? 'img' : 'span');
      thumb.className = 'thumb';
      if (thumb instanceof HTMLImageElement && e.thumb) {
        thumb.src = e.thumb;
        thumb.alt = '';
      }
      const text = document.createElement('div');
      text.className = 'text';
      const name = Object.assign(document.createElement('span'), { className: 'name', textContent: e.file.name });
      const detail = Object.assign(document.createElement('span'), {
        className: 'detail',
        textContent: `${formatTime(e.info.duration)} · ${e.info.width}×${e.info.height} · ${e.info.videoCodec}${e.info.audio ? '' : ' · no audio'}`,
      });
      text.append(name, detail);
      li.append(
        thumb,
        text,
        button('up', '↑', 'Move up', () => move(i, i - 1)),
        button('down', '↓', 'Move down', () => move(i, i + 1)),
        button('remove', '✕', 'Remove', () => removeEntry(i)),
      );
      return li;
    }),
  );
}

function renderTabs() {
  $('tabs').replaceChildren(
    ...state.entries.map((e, i) => {
      const bad = typeof entryTrim(e) === 'string';
      const b = Object.assign(document.createElement('button'), {
        type: 'button',
        textContent: bad ? `${e.file.name} (!)` : e.file.name,
      });
      b.dataset.index = String(i);
      b.setAttribute('role', 'tab');
      b.setAttribute('aria-selected', String(i === state.selected));
      b.classList.toggle('selected', i === state.selected);
      b.classList.toggle('invalid', bad);
      b.addEventListener('click', () => select(i));
      return b;
    }),
  );
}

export function refresh() {
  const e = current();
  const trim = e ? entryTrim(e) : null;
  message('trim-error', typeof trim === 'string' ? trim : null);
  const valid = state.entries.every((x) => typeof entryTrim(x) !== 'string');
  const reach = reachable(state.entries.length, valid, output.rom !== null);
  const convert = $<HTMLButtonElement>('convert');
  convert.disabled = busy.converting || !reach.build;
  convert.hidden = busy.converting;
  $<HTMLButtonElement>('next-adjust').disabled = !reach.adjust;
  $<HTMLButtonElement>('next-build').disabled = !reach.build;
  if (e) {
    refreshSubs(e);
    const b = bounds(e);
    placeTrim(b.start, b.end, e.info.duration);
  }
  $('cart-row').hidden = state.entries.length < 2;
  $('list-panel').hidden = state.entries.length === 0;
  renderList();
  renderTabs();
  render(reach);
  if (step() !== 'adjust') $<HTMLVideoElement>('video').pause();
  preview?.input(step() === 'play' && !isFormField(document.activeElement));
}

function select(i: number) {
  state.selected = i;
  const e = current();
  const video = $<HTMLVideoElement>('video');
  if (video.src) URL.revokeObjectURL(video.src);
  $<HTMLInputElement>('subs').value = '';
  if (!e) {
    video.removeAttribute('src');
    message('notice', null);
    refresh();
    return;
  }
  $<HTMLInputElement>('title').value = e.title;
  $<HTMLInputElement>('start').value = e.start;
  $<HTMLInputElement>('end').value = e.end;
  $('file-info').textContent = `${e.file.name} · ${e.info.width}×${e.info.height} · ${formatTime(e.info.duration)} · ${e.info.videoCodec}`;
  video.src = URL.createObjectURL(e.file);
  message('notice', e.info.audio ? null : 'No usable audio track. The cartridge will be silent.');
  refresh();
}

function edited() {
  if (busy.converting) void import('./convert').then((c) => c.cancel());
  if (output.rom) {
    output.rom = null;
    preview?.stop();
  }
}

function move(from: number, to: number) {
  if (to < 0 || to >= state.entries.length) return;
  edited();
  const list = state.entries;
  [list[from], list[to]] = [list[to], list[from]];
  if (state.selected === from) state.selected = to;
  else if (state.selected === to) state.selected = from;
  refresh();
}

function removeEntry(i: number) {
  edited();
  state.entries.splice(i, 1);
  if (i < state.selected) {
    state.selected--;
    refresh();
  } else if (i === state.selected) {
    select(state.entries.length === 0 ? -1 : Math.min(i, state.entries.length - 1));
  } else {
    refresh();
  }
}

function newCart() {
  edited();
  state.entries = [];
  $<HTMLInputElement>('cart-title').value = 'vid2gba';
  message('error', null);
  go('videos');
  select(-1);
}

let preview: Preview | null = null;

function isFormField(el: Element | null): boolean {
  return el instanceof HTMLInputElement || el instanceof HTMLSelectElement || el instanceof HTMLTextAreaElement;
}

async function showPreview(rom: Uint8Array) {
  try {
    preview ??= await Preview.create($<HTMLCanvasElement>('screen'));
    preview.load(rom);
    preview.input(step() === 'play' && !isFormField(document.activeElement));
    preview.volume(65);
  } catch (e) {
    message('error', `Preview unavailable: ${(e as Error).message}`);
  }
}

async function addFiles(files: File[]) {
  edited();
  message('error', null);
  let first = -1;
  for (const file of files) {
    const info = await inspect(file);
    if ('error' in info) {
      message('error', info.error);
      continue;
    }
    const entry: Entry = { file, info, title: file.name.replace(/\.[^.]+$/, ''), start: '', end: '', subsText: null, subsError: null, subsName: null, thumb: null };
    state.entries.push(entry);
    if (first < 0) first = state.entries.length - 1;
    void thumbnail(file, info.duration * 0.1).then((t) => {
      entry.thumb = t;
      renderList();
    });
  }
  if (first >= 0) select(first);
  else refresh();
}

function wire() {
  mountTrim(
    () => current()?.info.duration ?? 0,
    (w) => {
      const e = current();
      return e ? bounds(e)[w] : 0;
    },
    setTrim,
  );
  mountGba($('gba'), $<HTMLCanvasElement>('screen'), (k) => preview?.press(k), (k) => preview?.release(k));
  const input = $<HTMLInputElement>('file');
  input.addEventListener('change', () => {
    const files = [...(input.files ?? [])];
    input.value = '';
    if (files.length) void addFiles(files);
  });
  const drop = $('drop');
  drop.addEventListener('dragover', (e) => {
    e.preventDefault();
    drop.classList.add('over');
  });
  drop.addEventListener('dragleave', () => drop.classList.remove('over'));
  drop.addEventListener('drop', (e) => {
    e.preventDefault();
    drop.classList.remove('over');
    const files = [...(e.dataTransfer?.files ?? [])];
    if (files.length) void addFiles(files);
  });
  const field = (id: 'title' | 'start' | 'end') => {
    const el = $<HTMLInputElement>(id);
    el.addEventListener('input', () => {
      const e = current();
      if (!e) return;
      edited();
      e[id] = el.value;
      refresh();
    });
  };
  field('title');
  field('start');
  field('end');
  const subs = $<HTMLInputElement>('subs');
  subs.addEventListener('change', async () => {
    const e = current();
    const f = subs.files?.[0];
    if (!e) return;
    edited();
    e.subsText = null;
    e.subsError = null;
    e.subsName = f?.name ?? null;
    if (f) {
      try {
        e.subsText = new TextDecoder('utf-8', { fatal: true }).decode(await f.arrayBuffer());
      } catch {
        e.subsError = `${f.name} is not UTF-8 text; re-save it as UTF-8`;
      }
    }
    refresh();
  });
  $('subs-clear').addEventListener('click', () => {
    const e = current();
    subs.value = '';
    if (!e) return;
    edited();
    e.subsText = null;
    e.subsError = null;
    e.subsName = null;
    refresh();
  });
  const setTime = (id: 'start' | 'end') => {
    const e = current();
    if (!e) return;
    edited();
    e[id] = formatTime($<HTMLVideoElement>('video').currentTime);
    $<HTMLInputElement>(id).value = e[id];
    refresh();
  };
  $('set-start').addEventListener('click', () => setTime('start'));
  $('set-end').addEventListener('click', () => setTime('end'));
  const to = (s: Step) => () => {
    go(s);
    refresh();
  };
  for (const b of document.querySelectorAll<HTMLButtonElement>('#steps button')) b.addEventListener('click', to(b.dataset.step as Step));
  $('next-adjust').addEventListener('click', to('adjust'));
  $('back-videos').addEventListener('click', to('videos'));
  $('next-build').addEventListener('click', to('build'));
  $('back-adjust').addEventListener('click', to('adjust'));
  $('new-cart').addEventListener('click', newCart);
  for (const el of document.querySelectorAll<HTMLInputElement | HTMLSelectElement>('#cart-title, #fps, #half, input[name="target"]')) {
    el.addEventListener(el.id === 'cart-title' ? 'input' : 'change', () => {
      edited();
      refresh();
    });
  }
  $('convert').addEventListener('click', () => void import('./convert').then((c) => c.startConvert()));
  $('cancel').addEventListener('click', () => void import('./convert').then((c) => c.cancel()));
  $('download').addEventListener('click', () => void import('./convert').then((c) => c.download()));
  void import('./convert').then((c) => {
    c.onDone.handler = (rom) => {
      go('play');
      refresh();
      void showPreview(rom);
    };
  });
  document.addEventListener('focusin', (e) => {
    if (isFormField(e.target as Element)) preview?.input(false);
  });
  document.addEventListener('focusout', (e) => {
    if (isFormField(e.target as Element) && !isFormField(e.relatedTarget as Element | null)) preview?.input(step() === 'play');
  });
}

async function main() {
  wire();
  const missing = missingFeatures();
  if (missing.length) {
    $('missing').textContent = missing.join(', ');
    $('unsupported').hidden = false;
    return;
  }
  await init();
  $('app').hidden = false;
  $('steps').hidden = false;
  refresh();
}

void main();
