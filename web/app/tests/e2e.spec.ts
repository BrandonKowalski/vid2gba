import { expect, test, type Page } from '@playwright/test';
import path from 'node:path';
import { readFileSync } from 'node:fs';

const fixture = (n: string) => path.resolve('tests/fixtures', n);
const out = process.env.E2E_OUT ?? path.resolve('test-results/roms');

async function addVideos(page: Page, ...names: string[]) {
  await page.goto('/');
  await page.setInputFiles('#file', names.map(fixture));
  await expect(page.locator('#videos li')).toHaveCount(names.length);
}

async function goStep(page: Page, s: string) {
  await page.click(`#steps [data-step="${s}"]`);
  await expect(page.locator(`#step-${s}`)).toBeVisible();
}

async function toAdjust(page: Page) {
  await page.click('#next-adjust');
  await expect(page.locator('#step-adjust')).toBeVisible();
}

async function toBuild(page: Page) {
  await page.click('#next-build');
  await expect(page.locator('#step-build')).toBeVisible();
}

async function build(page: Page) {
  await page.click('#convert');
  await expect(page.locator('#step-play')).toBeVisible({ timeout: 180_000 });
}

async function save(page: Page, name: string) {
  const [dl] = await Promise.all([page.waitForEvent('download'), page.click('#download')]);
  await dl.saveAs(path.join(out, name));
  return dl;
}

async function convertAndSave(page: Page, file: string, name: string) {
  await addVideos(page, file);
  await toAdjust(page);
  await toBuild(page);
  await build(page);
  return save(page, name);
}

test('loads in a supported browser', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('#app')).toBeVisible();
  await expect(page.locator('#unsupported')).toBeHidden();
  expect(await page.evaluate(() => crossOriginIsolated)).toBe(true);
});

test('footer credits third-party licences on every step', async ({ page }) => {
  await addVideos(page, 'clip.webm');
  for (const s of ['videos', 'adjust', 'build']) {
    await goStep(page, s);
    const f = page.locator('#credits');
    await expect(f).toBeVisible();
    await expect(f).toContainText('MPL-2.0');
    await expect(f).toContainText('SIL OFL 1.1');
    await expect(f).toContainText('Mark Davis from the Noun Project, CC BY 3.0');
    await expect(f).not.toContainText('MIT');
    await expect(f.locator('a[href="https://github.com/BrandonKowalski/vid2gba"]')).toHaveText('Source on GitHub');
  }
  await expect(page.locator('#licenses-open')).toHaveCount(0);
});

test('page text has no em dashes or emoji', async ({ page }) => {
  await addVideos(page, 'silent.webm');
  await toAdjust(page);
  const text = await page.evaluate(
    () =>
      document.body.textContent +
      [...document.querySelectorAll('[aria-label],[placeholder],[title]')]
        .map((e) => `${e.getAttribute('aria-label')} ${e.getAttribute('placeholder')} ${e.getAttribute('title')}`)
        .join(' '),
  );
  expect(text).not.toMatch(/\u2014/);
  expect(text.replace('\u270C\u{1F3FB}', '')).not.toMatch(/\p{Extended_Pictographic}/u);
});

test('rejects non-video files', async ({ page }) => {
  await page.goto('/');
  await page.setInputFiles('#file', fixture('notvideo.txt'));
  await expect(page.locator('#error')).toContainText("Couldn't read this file as a video.");
  await expect(page.locator('#next-adjust')).toBeDisabled();
});

test('step bar only allows reached steps', async ({ page }) => {
  await page.goto('/');
  await expect(page.locator('#next-adjust')).toBeDisabled();
  for (const s of ['adjust', 'build', 'play']) await expect(page.locator(`#steps [data-step="${s}"]`)).toBeDisabled();
  await page.setInputFiles('#file', fixture('clip.webm'));
  await expect(page.locator('#steps [data-step="adjust"]')).toBeEnabled();
  await expect(page.locator('#steps [data-step="build"]')).toBeEnabled();
  await expect(page.locator('#steps [data-step="play"]')).toBeDisabled();
  await goStep(page, 'build');
});

test('shows file info and defaults', async ({ page }) => {
  await addVideos(page, 'clip.webm');
  await expect(page.locator('#videos li .detail')).toContainText('320×180');
  await toAdjust(page);
  await expect(page.locator('#file-info')).toContainText('320×180');
  await expect(page.locator('#title')).toHaveValue('clip');
  await expect(page.locator('#notice')).toBeHidden();
});

test('silent video gets a notice', async ({ page }) => {
  await addVideos(page, 'silent.webm');
  await expect(page.locator('#videos li .detail')).toContainText('no audio');
  await toAdjust(page);
  await expect(page.locator('#notice')).toHaveText('No usable audio track. The cartridge will be silent.');
});

test('start past the end blocks the build step', async ({ page }) => {
  await addVideos(page, 'clip.webm');
  await toAdjust(page);
  await page.fill('#start', '0:30');
  await expect(page.locator('#trim-error')).toContainText('past the end');
  await expect(page.locator('#next-build')).toBeDisabled();
  await expect(page.locator('#steps [data-step="build"]')).toBeDisabled();
  await page.fill('#start', '0:01');
  await expect(page.locator('#trim-error')).toBeHidden();
  await expect(page.locator('#next-build')).toBeEnabled();
});

test('converts a video and downloads it', async ({ page }) => {
  const dl = await convertAndSave(page, 'clip.webm', 'clip.gba');
  expect(dl.suggestedFilename()).toBe('clip.gba');
  await expect(page.locator('#summary')).toContainText('240×134 @ 20 fps');
});

test('converts a silent video', async ({ page }) => {
  await convertAndSave(page, 'silent.webm', 'silent.gba');
});

test('converts a trimmed segment', async ({ page }) => {
  await addVideos(page, 'clip.webm');
  await toAdjust(page);
  await page.fill('#start', '0:01');
  await page.fill('#end', '0:03');
  await toBuild(page);
  await build(page);
  await save(page, 'trimmed.gba');
});

test('non-ASCII title downloads a safe name', async ({ page }) => {
  await addVideos(page, 'clip.webm');
  await toAdjust(page);
  await page.fill('#title', 'Café ☕ clip');
  await toBuild(page);
  await build(page);
  const [dl] = await Promise.all([page.waitForEvent('download'), page.click('#download')]);
  expect(dl.suggestedFilename()).toBe('Caf_clip.gba');
});

test('cancel stops conversion', async ({ page }) => {
  await addVideos(page, 'clip.webm');
  await toAdjust(page);
  await toBuild(page);
  await page.click('#convert');
  await expect(page.locator('#progress')).toBeVisible();
  await expect(page.locator('#convert')).toBeHidden();
  await page.click('#cancel');
  await expect(page.locator('#progress')).toBeHidden();
  await expect(page.locator('#convert')).toBeEnabled();
  await expect(page.locator('#steps [data-step="play"]')).toBeDisabled();
});

test('preview plays the cartridge', async ({ page }) => {
  await convertAndSave(page, 'clip.webm', 'preview.gba');
  const canvas = page.locator('#screen');
  const first = await canvas.screenshot();
  let changed = false;
  for (let i = 0; i < 20 && !changed; i++) {
    await page.waitForTimeout(200);
    changed = !(await canvas.screenshot()).equals(first);
  }
  expect(changed).toBe(true);
});

test('second conversion replaces preview and notice', async ({ page }) => {
  await convertAndSave(page, 'silent.webm', 'first.gba');
  await goStep(page, 'videos');
  await page.setInputFiles('#file', fixture('clip.webm'));
  await expect(page.locator('#videos li')).toHaveCount(2);
  await expect(page.locator('#steps [data-step="play"]')).toBeDisabled();
  await toAdjust(page);
  await expect(page.locator('#notice')).toBeHidden();
  await toBuild(page);
  await build(page);
  await expect(page.locator('#summary')).toContainText('@ 20 fps');
});

test('on-screen START starts playback', async ({ page }) => {
  await convertAndSave(page, 'clip.webm', 'buttons.gba');
  await page.waitForTimeout(4500);
  await page.click('#gba [data-key="Start"]');
  await page.waitForTimeout(1000);
  const canvas = page.locator('#screen');
  const a = await canvas.screenshot();
  await page.waitForTimeout(500);
  const b = await canvas.screenshot();
  expect(a.equals(b)).toBe(false);
});

test('text fields accept typing after the preview starts', async ({ page }) => {
  await convertAndSave(page, 'clip.webm', 'typing.gba');
  await page.waitForTimeout(1500);
  await goStep(page, 'adjust');
  await page.click('#title');
  await page.keyboard.press('End');
  await page.keyboard.type('X');
  await page.keyboard.press('Backspace');
  await page.keyboard.type('YZ');
  await expect(page.locator('#title')).toHaveValue('clipYZ');
  await page.click('#end');
  await page.keyboard.type('0:04');
  await expect(page.locator('#end')).toHaveValue('0:04');
});

test('keyboard drives the emulator again after leaving a field', async ({ page }) => {
  await convertAndSave(page, 'clip.webm', 'keys.gba');
  await goStep(page, 'adjust');
  await page.click('#title');
  await goStep(page, 'play');
  await page.click('#screen');
  await page.waitForTimeout(4500);
  await page.keyboard.down('Enter');
  await page.waitForTimeout(150);
  await page.keyboard.up('Enter');
  await page.waitForTimeout(1000);
  const canvas = page.locator('#screen');
  const a = await canvas.screenshot();
  await page.waitForTimeout(500);
  const b = await canvas.screenshot();
  expect(a.equals(b)).toBe(false);
});

test('converts a video that starts late', async ({ page }) => {
  await convertAndSave(page, 'late-video.webm', 'late-video.gba');
});

test('converts audio that starts late', async ({ page }) => {
  await convertAndSave(page, 'late-audio.webm', 'late-audio.gba');
});

test('editing after a build locks play', async ({ page }) => {
  await addVideos(page, 'clip.webm');
  await toAdjust(page);
  await page.fill('#title', 'First');
  await toBuild(page);
  await build(page);
  await goStep(page, 'adjust');
  await expect(page.locator('#steps [data-step="play"]')).toBeEnabled();
  await page.fill('#title', 'Second');
  await expect(page.locator('#steps [data-step="play"]')).toBeDisabled();
  await goStep(page, 'build');
  await build(page);
  const [dl] = await Promise.all([page.waitForEvent('download'), page.click('#download')]);
  expect(dl.suggestedFilename()).toBe('Second.gba');
});

test('opening a new file stops the old preview', async ({ page }) => {
  await convertAndSave(page, 'clip.webm', 'stop.gba');
  await page.waitForTimeout(4500);
  await page.click('#gba [data-key="Start"]');
  await page.waitForTimeout(800);
  await goStep(page, 'videos');
  await page.setInputFiles('#file', fixture('silent.webm'));
  await expect(page.locator('#steps [data-step="play"]')).toBeDisabled();
  await page.evaluate(() => {
    document.getElementById('step-play')!.hidden = false;
  });
  await page.waitForTimeout(300);
  const canvas = page.locator('#screen');
  const a = await canvas.screenshot();
  await page.waitForTimeout(600);
  const b = await canvas.screenshot();
  expect(a.equals(b)).toBe(true);
});

test('converts a detailed 1080p video', async ({ page }) => {
  await convertAndSave(page, 'detail.webm', 'detail.gba');
});

test('adds subtitles to the cartridge', async ({ page }) => {
  await addVideos(page, 'clip.webm');
  await toAdjust(page);
  await page.setInputFiles('#subs', fixture('subs.vtt'));
  await expect(page.locator('#subs-info')).toHaveText('subs.vtt');
  await toBuild(page);
  await build(page);
  await save(page, 'subs.gba');
});

test('bad subtitles show an error but conversion still works', async ({ page }) => {
  await addVideos(page, 'clip.webm');
  await toAdjust(page);
  await page.setInputFiles('#subs', fixture('bad.srt'));
  await expect(page.locator('#subs-info')).toContainText('line 2');
  await expect(page.locator('#next-build')).toBeEnabled();
  await page.click('#subs-clear');
  await expect(page.locator('#subs-info')).toBeHidden();
});

test('non-UTF-8 subtitles are rejected with a clear message', async ({ page }) => {
  await addVideos(page, 'clip.webm');
  await toAdjust(page);
  await page.setInputFiles('#subs', fixture('latin1.srt'));
  await expect(page.locator('#subs-info')).toContainText('UTF-8');
  await expect(page.locator('#subs-info')).toHaveClass('error');
  await expect(page.locator('#next-build')).toBeEnabled();
});

test('on-screen D-pad seeks', async ({ page }) => {
  await convertAndSave(page, 'clip.webm', 'dpad.gba');
  await page.waitForTimeout(4500);
  await page.locator('#gba [data-key="Start"]').dispatchEvent('pointerdown');
  await page.waitForTimeout(150);
  await page.locator('#gba [data-key="Start"]').dispatchEvent('pointerup');
  await page.waitForTimeout(600);
  await page.locator('#screen').scrollIntoViewIfNeeded();
  const box = (await page.locator('#screen').boundingBox())!;
  const strip = { x: box.x, y: box.y + box.height * 0.92, width: box.width, height: box.height * 0.06 };
  const before = await page.screenshot({ clip: strip });
  await page.locator('#gba [data-key="Right"]').dispatchEvent('pointerdown');
  await page.waitForTimeout(150);
  await page.locator('#gba [data-key="Right"]').dispatchEvent('pointerup');
  await page.waitForTimeout(400);
  const after = await page.screenshot({ clip: strip });
  expect(before.equals(after)).toBe(false);
});

test('builds a multi-video cartridge from a list', async ({ page }) => {
  await addVideos(page, 'clip.webm', 'silent.webm', 'late-video.webm');
  await expect(page.locator('#cart-row')).toBeVisible();
  await page.locator('#videos li').nth(2).locator('.up').click();
  await expect(page.locator('#videos li').nth(1).locator('.name')).toContainText('late-video');
  await page.locator('#videos li').nth(0).locator('.remove').click();
  await expect(page.locator('#videos li')).toHaveCount(2);
  await page.fill('#cart-title', 'Set');
  await toAdjust(page);
  await page.locator('#tabs button').nth(1).click();
  await page.fill('#title', 'Quiet one');
  await toBuild(page);
  await build(page);
  await expect(page.locator('#summary')).toContainText('2 videos');
  const dl = await save(page, 'set.gba');
  expect(dl.suggestedFilename()).toBe('Set.gba');
});

test('invalid trim on one entry blocks the build step', async ({ page }) => {
  await addVideos(page, 'clip.webm', 'silent.webm');
  await toAdjust(page);
  await page.locator('#tabs button').nth(1).click();
  await page.fill('#start', '0:30');
  await expect(page.locator('#next-build')).toBeDisabled();
  await expect(page.locator('#tabs button').nth(1)).toHaveClass(/invalid/);
  await expect(page.locator('#tabs button').nth(1)).toContainText('(!)');
  await expect(page.locator('#tabs button').nth(0)).not.toHaveClass(/invalid/);
  await page.locator('#tabs button').nth(0).click();
  await expect(page.locator('#next-build')).toBeDisabled();
  await page.locator('#tabs button').nth(1).click();
  await page.fill('#start', '');
  await expect(page.locator('#next-build')).toBeEnabled();
});

test('remove selected video selects neighbour', async ({ page }) => {
  await addVideos(page, 'clip.webm', 'silent.webm');
  await toAdjust(page);
  await page.locator('#tabs button').nth(1).click();
  await goStep(page, 'videos');
  await page.locator('#videos li').nth(1).locator('.remove').click();
  await expect(page.locator('#videos li')).toHaveCount(1);
  await expect(page.locator('#cart-row')).toBeHidden();
  await toAdjust(page);
  await expect(page.locator('#title')).toHaveValue('clip');
});

test('removing another video keeps the selection', async ({ page }) => {
  await addVideos(page, 'clip.webm', 'silent.webm', 'late-video.webm');
  await toAdjust(page);
  await page.locator('#tabs button').nth(2).click();
  await goStep(page, 'videos');
  await page.locator('#videos li').nth(0).locator('.remove').click();
  await expect(page.locator('#videos li')).toHaveCount(2);
  await toAdjust(page);
  await expect(page.locator('#title')).toHaveValue('late-video');
  await expect(page.locator('#tabs button').nth(1)).toHaveClass(/selected/);
});

test('changing the list cancels a running conversion', async ({ page }) => {
  await addVideos(page, 'clip.webm', 'silent.webm');
  await toAdjust(page);
  await toBuild(page);
  await page.click('#convert');
  await expect(page.locator('#progress')).toBeVisible();
  await goStep(page, 'videos');
  await page.locator('#videos li').nth(1).locator('.remove').click();
  await goStep(page, 'build');
  await expect(page.locator('#progress')).toBeHidden();
  await expect(page.locator('#convert')).toBeEnabled();
  await page.waitForTimeout(20_000);
  await expect(page.locator('#steps [data-step="play"]')).toBeDisabled();
});

test('start new cart clears everything', async ({ page }) => {
  await addVideos(page, 'clip.webm');
  await toAdjust(page);
  await toBuild(page);
  await build(page);
  await page.click('#new-cart');
  await expect(page.locator('#step-videos')).toBeVisible();
  await expect(page.locator('#videos li')).toHaveCount(0);
  await expect(page.locator('#next-adjust')).toBeDisabled();
  for (const s of ['adjust', 'build', 'play']) await expect(page.locator(`#steps [data-step="${s}"]`)).toBeDisabled();
});

test('the console has labelled buttons and no volume control', async ({ page }) => {
  await convertAndSave(page, 'clip.webm', 'console.gba');
  const keys = page.locator('#gba [data-key]');
  await expect(keys).toHaveCount(10);
  for (const k of ['Up', 'Down', 'Left', 'Right', 'A', 'B', 'L', 'R', 'Start', 'Select']) {
    await expect(page.locator(`#gba [data-key="${k}"]`)).toHaveAttribute('aria-label', /.+/);
  }
  await expect(page.locator('#gba .gba-screen #screen')).toBeVisible();
  await expect(page.locator('#volume')).toHaveCount(0);
  await expect(page.locator('.pad')).toHaveCount(0);
});

test('dragging the trim handles sets the times', async ({ page }) => {
  await addVideos(page, 'clip.webm');
  await toAdjust(page);
  const track = (await page.locator('#trim').boundingBox())!;
  const drag = async (id: string, to: number) => {
    const h = (await page.locator(id).boundingBox())!;
    await page.mouse.move(h.x + h.width / 2, h.y + h.height / 2);
    await page.mouse.down();
    await page.mouse.move(track.x + track.width * to, h.y + h.height / 2, { steps: 5 });
    await page.mouse.up();
  };
  await drag('#trim-start', 0.5);
  await expect(page.locator('#start')).toHaveValue(/^0:0[23]\.\d$/);
  await drag('#trim-start', -0.2);
  await expect(page.locator('#start')).toHaveValue('');
  await page.fill('#end', '0:04.5');
  const left = await page.locator('#trim-end').evaluate((el) => parseFloat((el as HTMLElement).style.left));
  expect(left).toBeCloseTo(75, 0);
  await page.locator('#trim-end').focus();
  await page.keyboard.press('ArrowLeft');
  await expect(page.locator('#end')).toHaveValue('0:04.4');
  await drag('#trim-end', 1.2);
  await expect(page.locator('#end')).toHaveValue('');
  await expect(page.locator('#next-build')).toBeEnabled();
});

test('uses the bundled fonts and uppercase headings', async ({ page }) => {
  const external: string[] = [];
  page.on('request', (r) => {
    if (/fonts\.(googleapis|gstatic)\.com/.test(r.url())) external.push(r.url());
  });
  await page.goto('/');
  await page.evaluate(() => document.fonts.ready);
  expect(await page.locator('h1').evaluate((el) => getComputedStyle(el).fontFamily)).toContain('Press Start 2P');
  expect(await page.locator('body').evaluate((el) => getComputedStyle(el).fontFamily)).toContain('IBM Plex Mono');
  expect(await page.evaluate(() => [...document.fonts].some((f) => f.family.includes('Press Start 2P') && f.status === 'loaded'))).toBe(true);
  expect(await page.locator('#step-videos h2').evaluate((el) => getComputedStyle(el).textTransform)).toBe('uppercase');
  expect(external).toEqual([]);
});

test('fits a phone screen on every step', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 800 });
  const fits = () => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth);
  await addVideos(page, 'clip.webm', 'silent.webm');
  expect(await fits()).toBe(true);
  await toAdjust(page);
  expect(await fits()).toBe(true);
  await toBuild(page);
  expect(await fits()).toBe(true);
  await build(page);
  expect(await fits()).toBe(true);
});

test('times at a minute mark stay valid', async ({ page }) => {
  await addVideos(page, 'long.webm');
  await toAdjust(page);
  await page.evaluate(() => {
    (document.getElementById('video') as HTMLVideoElement).currentTime = 59.96;
  });
  await page.waitForFunction(() => Math.abs((document.getElementById('video') as HTMLVideoElement).currentTime - 59.96) < 0.01);
  await page.click('#set-start');
  await expect(page.locator('#start')).toHaveValue('1:00.0');
  await expect(page.locator('#trim-error')).toBeHidden();
});

test('changing build settings after a build locks play', async ({ page }) => {
  await addVideos(page, 'clip.webm', 'silent.webm');
  await toAdjust(page);
  await toBuild(page);
  await build(page);
  await goStep(page, 'build');
  await page.locator('input[name="target"][value="gba"]').check({ force: true });
  await expect(page.locator('#steps [data-step="play"]')).toBeDisabled();
  await build(page);
  await goStep(page, 'videos');
  await page.fill('#cart-title', 'Trip');
  await expect(page.locator('#steps [data-step="play"]')).toBeDisabled();
});

test('step buttons show the step name in capitals', async ({ page }) => {
  await addVideos(page, 'clip.webm');
  expect(await page.locator('#next-adjust').innerText()).toBe('ADJUST');
  await toAdjust(page);
  expect(await page.locator('#back-videos').innerText()).toBe('SELECT VIDEOS');
  expect(await page.locator('#steps [data-step="videos"]').innerText()).toBe('SELECT VIDEOS');
  expect(await page.locator('#next-build').innerText()).toBe('BUILD');
  await toBuild(page);
  expect(await page.locator('#back-adjust').innerText()).toBe('ADJUST');
  expect(await page.locator('#steps [data-step="build"]').innerText()).toBe('BUILD');
});

test('long file names do not widen a phone screen', async ({ page }) => {
  await page.setViewportSize({ width: 375, height: 800 });
  await page.goto('/');
  const name = 'VID_20260101_123456789_stabilized_export_final_version_two.webm';
  await page.setInputFiles('#file', { name, mimeType: 'video/webm', buffer: readFileSync(fixture('clip.webm')) });
  await expect(page.locator('#videos li')).toHaveCount(1);
  const fits = () => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth);
  expect(await fits()).toBe(true);
  await toAdjust(page);
  await page.setInputFiles('#subs', { name: 'VID_20260101_123456789_stabilized_export_final_version_two.srt', mimeType: 'text/plain', buffer: readFileSync(fixture('subs.vtt')) });
  expect(await fits()).toBe(true);
});

test('stays light when the system prefers dark', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto('/');
  const bg = await page.locator('body').evaluate((el) => getComputedStyle(el).backgroundColor);
  expect(bg).toBe('rgb(247, 245, 239)');
  expect(await page.locator('#steps [data-step="videos"]').evaluate((el) => getComputedStyle(el).color)).toBe('rgb(75, 63, 176)');
});
