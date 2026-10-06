export function missingFeatures(): string[] {
  const missing: string[] = [];
  if (!('VideoDecoder' in globalThis)) missing.push('WebCodecs video decoding');
  if (!('AudioDecoder' in globalThis)) missing.push('WebCodecs audio decoding');
  if (!globalThis.crossOriginIsolated) missing.push('cross-origin isolation');
  if (typeof OffscreenCanvas === 'undefined') missing.push('OffscreenCanvas');
  if (typeof WebAssembly === 'undefined') missing.push('WebAssembly');
  if (typeof Worker === 'undefined') missing.push('Web Workers');
  return missing;
}
