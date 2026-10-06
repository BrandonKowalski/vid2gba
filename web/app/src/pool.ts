import type { EncodeJob, EncodeResult } from './protocol';

export class Pool {
  readonly size: number;
  private all: Worker[] = [];
  private idle: Worker[] = [];
  private waiters: ((w: Worker) => void)[] = [];

  constructor(size: number) {
    this.size = size;
    for (let i = 0; i < size; i++) {
      const w = new Worker(new URL('./encode.worker.ts', import.meta.url), { type: 'module' });
      this.all.push(w);
      this.idle.push(w);
    }
  }

  private acquire(): Promise<Worker> {
    const w = this.idle.pop();
    return w ? Promise.resolve(w) : new Promise((resolve) => this.waiters.push(resolve));
  }

  private release(w: Worker) {
    const next = this.waiters.shift();
    if (next) next(w);
    else this.idle.push(w);
  }

  async encode(job: EncodeJob): Promise<Uint8Array> {
    const worker = await this.acquire();
    return new Promise((resolve, reject) => {
      worker.onmessage = (e: MessageEvent<EncodeResult>) => {
        this.release(worker);
        if ('out' in e.data) resolve(e.data.out);
        else reject(new Error(e.data.error));
      };
      worker.onerror = (e) => {
        this.release(worker);
        reject(new Error(e.message));
      };
      worker.postMessage(job, [job.gop.buffer]);
    });
  }

  terminate() {
    for (const w of this.all) w.terminate();
  }
}
