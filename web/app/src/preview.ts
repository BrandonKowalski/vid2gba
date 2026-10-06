interface Emulator {
  FSInit(): Promise<void>;
  FS: { writeFile(path: string, data: Uint8Array): void };
  filePaths(): { gamePath: string };
  loadGame(path: string): boolean;
  quitGame(): void;
  bindKey(key: string, input: string): void;
  buttonPress(name: string): void;
  buttonUnpress(name: string): void;
  setVolume(percent: number): void;
  resumeAudio(): void;
  toggleInput(enabled: boolean): void;
}

const BINDINGS: [string, string][] = [
  ['Return', 'Start'],
  ['Backspace', 'Select'],
  ['X', 'A'],
  ['Z', 'B'],
  ['A', 'L'],
  ['S', 'R'],
];

export class Preview {
  private loaded = false;

  private constructor(private emu: Emulator) {}

  static async create(canvas: HTMLCanvasElement): Promise<Preview> {
    const url = new URL('mgba/mgba.js', document.baseURI).href;
    const mod = await import(/* @vite-ignore */ url);
    const emu: Emulator = await mod.default({ canvas });
    await emu.FSInit();
    for (const [key, input] of BINDINGS) emu.bindKey(key, input);
    return new Preview(emu);
  }

  load(rom: Uint8Array) {
    if (this.loaded) this.emu.quitGame();
    const path = `${this.emu.filePaths().gamePath}/preview.gba`;
    this.emu.FS.writeFile(path, rom);
    if (!this.emu.loadGame(path)) throw new Error('the emulator rejected the ROM');
    this.loaded = true;
    this.emu.resumeAudio();
  }

  stop() {
    if (!this.loaded) return;
    this.emu.quitGame();
    this.loaded = false;
  }

  press(name: string) {
    this.emu.buttonPress(name);
  }

  release(name: string) {
    this.emu.buttonUnpress(name);
  }

  volume(percent: number) {
    this.emu.setVolume(percent);
  }

  input(enabled: boolean) {
    this.emu.toggleInput(enabled);
  }
}
