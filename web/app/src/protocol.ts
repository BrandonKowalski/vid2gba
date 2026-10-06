export interface EntryRequest {
  file: File;
  start: number;
  end: number;
  width: number;
  height: number;
  audio: boolean;
  title: string;
  subtitles: string | null;
}

export interface ConvertRequest {
  entries: EntryRequest[];
  cartTitle: string;
  fps: number | null;
  halfRes: boolean;
  realGba: boolean;
}

export type WorkerMessage =
  | { type: 'progress'; fraction: number; label: string }
  | { type: 'error'; message: string }
  | { type: 'done'; rom: Uint8Array; summary: string; realGba: boolean };

export interface EncodeJob {
  gop: Uint8Array;
  w: number;
  h: number;
  codebook: number;
  skip: number;
}

export type EncodeResult = { out: Uint8Array } | { error: string };
