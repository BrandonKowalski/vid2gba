import { execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';

export default function setup() {
  mkdirSync('tests/fixtures', { recursive: true });
  const src = ['-v', 'error', '-y', '-f', 'lavfi', '-i', 'testsrc=size=320x180:rate=30:duration=6'];
  execFileSync('ffmpeg', [...src, '-f', 'lavfi', '-i', 'sine=frequency=440:duration=6', '-c:v', 'libvpx-vp9', '-b:v', '400k', '-c:a', 'libopus', 'tests/fixtures/clip.webm']);
  execFileSync('ffmpeg', [...src, '-an', '-c:v', 'libvpx-vp9', '-b:v', '400k', 'tests/fixtures/silent.webm']);
  execFileSync('ffmpeg', ['-v', 'error', '-y', '-itsoffset', '0.5', '-f', 'lavfi', '-i', 'testsrc=size=320x180:rate=30:duration=5.5', '-f', 'lavfi', '-i', 'sine=frequency=440:duration=6', '-c:v', 'libvpx-vp9', '-b:v', '400k', '-c:a', 'libopus', 'tests/fixtures/late-video.webm']);
  execFileSync('ffmpeg', [...src, '-itsoffset', '1.0', '-f', 'lavfi', '-i', 'sine=frequency=440:duration=5', '-c:v', 'libvpx-vp9', '-b:v', '400k', '-c:a', 'libopus', 'tests/fixtures/late-audio.webm']);
  execFileSync('ffmpeg', ['-v', 'error', '-y', '-f', 'lavfi', '-i', 'cellauto=s=1920x1080:rule=110:rate=30', '-t', '2', '-c:v', 'libvpx-vp9', '-b:v', '8M', '-pix_fmt', 'yuv420p', 'tests/fixtures/detail.webm']);
  execFileSync('ffmpeg', ['-v', 'error', '-y', '-f', 'lavfi', '-i', 'testsrc=size=160x90:rate=10:duration=61', '-an', '-c:v', 'libvpx-vp9', '-b:v', '50k', 'tests/fixtures/long.webm']);
  writeFileSync('tests/fixtures/subs.vtt', 'WEBVTT\n\n00:01.000 --> 00:02.000\nFirst line here\n\n00:03.000 --> 00:04.000\nSecond line here\n');
  writeFileSync('tests/fixtures/bad.srt', '1\n00:00:01 --> 00:00:02,000\nx\n');
  writeFileSync('tests/fixtures/latin1.srt', Buffer.from('1\n00:00:01,000 --> 00:00:02,000\ncaf\xe9\n', 'latin1'));
  writeFileSync('tests/fixtures/notvideo.txt', 'this is not a video');
}
