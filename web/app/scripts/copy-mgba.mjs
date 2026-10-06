import { cpSync, mkdirSync } from 'node:fs';

mkdirSync('public/mgba', { recursive: true });
for (const f of ['mgba.js', 'mgba.wasm']) {
  cpSync(`node_modules/@thenick775/mgba-wasm/dist/${f}`, `public/mgba/${f}`);
}
