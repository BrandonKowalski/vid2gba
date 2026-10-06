import init, { encode_gop_packed } from './wasm-pkg/vid2gba.js';
import type { EncodeJob, EncodeResult } from './protocol';

const ready = init();

function reply(m: EncodeResult, transfer: Transferable[] = []) {
  (self as unknown as Worker).postMessage(m, transfer);
}

self.onmessage = async (e: MessageEvent<EncodeJob>) => {
  try {
    await ready;
    const { gop, w, h, codebook, skip } = e.data;
    const out = encode_gop_packed(gop, w, h, codebook, skip);
    reply({ out }, [out.buffer]);
  } catch (err) {
    reply({ error: err instanceof Error ? err.message : String(err) });
  }
};
