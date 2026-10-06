#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use vid2gba_core::decode::Decoder;
use vid2gba_core::frame::Layout;

pub fn make_clip(dir: &Path, audio: bool, seconds: u32) -> PathBuf {
    let path = dir.join(if audio { "clip.mkv" } else { "silent.mkv" });
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-v", "error", "-y", "-f", "lavfi", "-i", &format!("testsrc=size=320x180:rate=30:duration={seconds}")]);
    if audio {
        cmd.args(["-f", "lavfi", "-i", &format!("sine=frequency=440:duration={seconds}"), "-c:a", "pcm_s16le"]);
    }
    cmd.args(["-c:v", "mpeg4", "-q:v", "4"]).arg(&path);
    assert!(cmd.status().unwrap().success());
    path
}

pub fn convert(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vid2gba")).args(args).output().unwrap()
}

pub fn harness() -> PathBuf {
    std::env::var("MGBA_SHOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/mgba-shot/mgba-shot"))
}

pub fn same_rows(a: &[u8], b: &[u8], skip: Option<std::ops::Range<usize>>) -> bool {
    a.len() == b.len()
        && a.chunks(240 * 3).zip(b.chunks(240 * 3)).enumerate().all(|(y, (ra, rb))| {
            skip.as_ref().is_some_and(|s| s.contains(&y)) || ra.iter().zip(rb).all(|(x, y)| x >> 3 == y >> 3)
        })
}

pub fn run(rom: &Path, dir: &Path, spec: &[&str]) -> Vec<Vec<u8>> {
    let status = Command::new(harness()).arg(rom).arg(dir).args(spec).status().expect("run tools/mgba-shot/build.sh");
    assert!(status.success());
    let n = spec.iter().filter(|s| !s.contains('@')).count();
    (1..=n).map(|i| std::fs::read(dir.join(format!("shot{i}.rgb"))).unwrap()).collect()
}

pub fn screens(layout: Layout, frames: &[&[u8]]) -> Vec<Vec<u8>> {
    let mut d = Decoder::new(layout);
    frames
        .iter()
        .map(|f| {
            d.decode(f);
            d.screen_rgb()
        })
        .collect()
}

pub fn find(screens: &[Vec<u8>], shot: &[u8], skip: Option<std::ops::Range<usize>>) -> Option<usize> {
    screens.iter().position(|s| same_rows(s, shot, skip.clone()))
}

pub fn make_source(dir: &Path, name: &str, lavfi: &str, seconds: u32) -> PathBuf {
    let path = dir.join(format!("{name}.mkv"));
    let ok = Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi", "-i", &format!("{lavfi}:duration={seconds}"), "-f", "lavfi", "-i", &format!("sine=frequency=440:duration={seconds}"), "-c:v", "mpeg4", "-q:v", "4", "-c:a", "pcm_s16le"])
        .arg(&path)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    path
}

