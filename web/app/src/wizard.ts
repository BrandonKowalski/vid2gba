export type Step = 'videos' | 'adjust' | 'build' | 'play';

const STEPS: Step[] = ['videos', 'adjust', 'build', 'play'];

let currentStep: Step = 'videos';

export function reachable(count: number, valid: boolean, hasRom: boolean): Record<Step, boolean> {
  const adjust = count > 0;
  const build = adjust && valid;
  return { videos: true, adjust, build, play: build && hasRom };
}

export function step(): Step {
  return currentStep;
}

export function go(s: Step) {
  currentStep = s;
}

export function render(reach: Record<Step, boolean>) {
  while (!reach[currentStep]) currentStep = STEPS[STEPS.indexOf(currentStep) - 1];
  for (const s of STEPS) {
    document.getElementById(`step-${s}`)!.hidden = s !== currentStep;
    const b = document.querySelector<HTMLButtonElement>(`#steps [data-step="${s}"]`)!;
    b.disabled = !reach[s];
    b.classList.toggle('current', s === currentStep);
    b.classList.toggle('done', reach[s] && s !== currentStep);
    if (s === currentStep) b.setAttribute('aria-current', 'step');
    else b.removeAttribute('aria-current');
  }
}
