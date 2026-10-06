mod common;

use common::{convert, make_clip};
use vid2gba_core::container::parse;
use vid2gba_core::decode::Decoder;
use vid2gba_core::frame::Layout;
use vid2gba_core::rom::{container_of, header_checksum};

#[test]
fn converts_clip_with_audio() {
    let dir = tempfile::tempdir().unwrap();
    let clip = make_clip(dir.path(), true, 3);
    let out = dir.path().join("out.gba");
    let res = convert(&[clip.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert!(res.status.success(), "{}", String::from_utf8_lossy(&res.stderr));
    let rom = std::fs::read(&out).unwrap();
    assert!(rom.len() <= 32 * 1024 * 1024);
    assert_eq!(rom[0xB2], 0x96);
    assert_eq!(rom[0xBD], header_checksum(&rom));
    let c = parse(container_of(&rom).unwrap()).unwrap();
    assert_eq!(c.layout, Layout::fit(320, 180, false));
    assert_eq!(c.fps, 20);
    assert!((58..=62).contains(&c.frames.len()), "{}", c.frames.len());
    assert!((c.audio_samples as i64 - 3 * 13379).abs() < 500);
    assert_eq!(c.title, "clip");
    let mut d = Decoder::new(c.layout);
    for f in &c.frames {
        d.decode(f);
    }
}

#[test]
fn converts_clip_without_audio() {
    let dir = tempfile::tempdir().unwrap();
    let clip = make_clip(dir.path(), false, 2);
    let out = dir.path().join("out.gba");
    let res = convert(&[clip.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert!(res.status.success(), "{}", String::from_utf8_lossy(&res.stderr));
    let rom = std::fs::read(&out).unwrap();
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let pcm = vid2gba_core::adpcm::decode(c.audio, c.audio_samples as usize);
    assert!(pcm.len() > 20000 && pcm.iter().all(|&s| s == 0));
}

#[test]
fn rejects_start_past_end() {
    let dir = tempfile::tempdir().unwrap();
    let clip = make_clip(dir.path(), true, 2);
    let res = convert(&[clip.to_str().unwrap(), "--start", "10", "-o", dir.path().join("x.gba").to_str().unwrap()]);
    assert!(!res.status.success());
    assert!(String::from_utf8_lossy(&res.stderr).contains("past the end"));
    let res = convert(&[clip.to_str().unwrap(), "--start", "1", "--end", "0:01"]);
    assert!(!res.status.success());
    assert!(String::from_utf8_lossy(&res.stderr).contains("must be after"));
}

#[test]
fn reports_when_too_big() {
    let dir = tempfile::tempdir().unwrap();
    let clip = make_clip(dir.path(), true, 2);
    let res = convert(&[clip.to_str().unwrap(), "--max-size", "150000", "-o", dir.path().join("x.gba").to_str().unwrap()]);
    assert!(!res.status.success());
    let err = String::from_utf8_lossy(&res.stderr);
    assert!(err.contains("doesn't fit") && err.contains("--start"), "{err}");
    assert!(err.contains("decoding video") && !err.contains("audio alone"), "{err}");
}

#[test]
fn fails_fast_when_audio_alone_is_too_big() {
    let dir = tempfile::tempdir().unwrap();
    let clip = make_clip(dir.path(), true, 2);
    let res = convert(&[clip.to_str().unwrap(), "--max-size", "100000", "-o", dir.path().join("x.gba").to_str().unwrap()]);
    assert!(!res.status.success());
    let err = String::from_utf8_lossy(&res.stderr);
    assert!(err.contains("audio alone") && err.contains("--start"), "{err}");
    assert!(!err.contains("decoding video"), "{err}");
}

#[test]
fn custom_non_ascii_title() {
    let dir = tempfile::tempdir().unwrap();
    let clip = make_clip(dir.path(), true, 1);
    let out = dir.path().join("t.gba");
    let res = convert(&[clip.to_str().unwrap(), "--title", "Café ☕", "--half-res", "--fps", "10", "-o", out.to_str().unwrap()]);
    assert!(res.status.success(), "{}", String::from_utf8_lossy(&res.stderr));
    let rom = std::fs::read(&out).unwrap();
    assert_eq!(&rom[0xA0..0xA4], b"CAF ");
    let c = parse(container_of(&rom).unwrap()).unwrap();
    assert_eq!(c.title, "Caf? ?");
    assert!(c.layout.half_res);
    assert_eq!(c.fps, 10);
}

#[test]
fn rejects_max_size_above_cartridge_limit() {
    let res = convert(&["whatever.mkv", "--max-size", "40000000"]);
    assert!(!res.status.success());
    let err = String::from_utf8_lossy(&res.stderr);
    assert!(err.contains("--max-size"), "{err}");
    assert!(!err.contains("not found"), "{err}");
}

#[test]
fn reports_missing_input_file() {
    let res = convert(&["/nonexistent/clip.mp4"]);
    assert!(!res.status.success());
    assert!(String::from_utf8_lossy(&res.stderr).contains("file not found"));
}

fn mean_brightness(rom: &[u8]) -> f64 {
    let c = parse(container_of(rom).unwrap()).unwrap();
    let mut d = Decoder::new(c.layout);
    for f in &c.frames {
        d.decode(f);
    }
    let rgb = d.active_rgb();
    rgb.iter().map(|&v| v as f64).sum::<f64>() / rgb.len() as f64
}

#[test]
fn target_gba_is_brighter() {
    let dir = tempfile::tempdir().unwrap();
    let clip = dir.path().join("gray.mkv");
    assert!(
        std::process::Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", "color=c=0x505050:size=320x180:rate=30:duration=1", "-c:v", "mpeg4"])
            .arg(&clip)
            .status()
            .unwrap()
            .success()
    );
    let emu = dir.path().join("emu.gba");
    let gba = dir.path().join("gba.gba");
    assert!(convert(&[clip.to_str().unwrap(), "-o", emu.to_str().unwrap()]).status.success());
    assert!(convert(&[clip.to_str().unwrap(), "--target", "gba", "-o", gba.to_str().unwrap()]).status.success());
    let (a, b) = (mean_brightness(&std::fs::read(&emu).unwrap()), mean_brightness(&std::fs::read(&gba).unwrap()));
    assert!(b > a + 10.0, "emulator {a}, gba {b}");
}

#[test]
fn rejects_unknown_target() {
    let res = convert(&["whatever.mkv", "--target", "GBA"]);
    assert!(!res.status.success());
    let err = String::from_utf8_lossy(&res.stderr);
    assert!(err.contains("emulator") && err.contains("gba"), "{err}");
}

fn write(dir: &std::path::Path, name: &str, text: &str) -> std::path::PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

const SRT: &str = "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n2\n00:00:02,500 --> 00:00:03,500\nWorld\n";

fn subtitle_count(rom: &std::path::Path) -> usize {
    let bytes = std::fs::read(rom).unwrap();
    parse(container_of(&bytes).unwrap()).unwrap().subtitles.len()
}

#[test]
fn subtitles_file_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let clip = make_clip(dir.path(), true, 4);
    let srt = write(dir.path(), "s.srt", SRT);
    let out = dir.path().join("o.gba");
    let res = convert(&[clip.to_str().unwrap(), "--subtitles", srt.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert!(res.status.success(), "{}", String::from_utf8_lossy(&res.stderr));
    let bytes = std::fs::read(&out).unwrap();
    let c = parse(container_of(&bytes).unwrap()).unwrap();
    assert_eq!(c.subtitles.len(), 2);
    assert_eq!(c.subtitles[0].start_sample, 13379);
}

#[test]
fn subtitles_follow_trim() {
    let dir = tempfile::tempdir().unwrap();
    let clip = make_clip(dir.path(), true, 4);
    let srt = write(dir.path(), "s.srt", SRT);
    let out = dir.path().join("o.gba");
    let res = convert(&[clip.to_str().unwrap(), "--subtitles", srt.to_str().unwrap(), "--start", "2", "-o", out.to_str().unwrap()]);
    assert!(res.status.success());
    let bytes = std::fs::read(&out).unwrap();
    let c = parse(container_of(&bytes).unwrap()).unwrap();
    assert_eq!(c.subtitles.len(), 1);
    assert_eq!(c.subtitles[0].start_sample, (0.5f64 * 13379.0).round() as u32);
}

fn mux(dir: &std::path::Path, codec: &str, ext: &str) -> std::path::PathBuf {
    let clip = make_clip(dir, true, 4);
    let srt = write(dir, "e.srt", SRT);
    let out = dir.join(format!("muxed.{ext}"));
    let ok = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i"])
        .arg(&clip)
        .arg("-i")
        .arg(&srt)
        .args(["-map", "0", "-map", "1", "-c:v", "copy", "-c:a", "aac", "-c:s", codec])
        .arg(&out)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    out
}

#[test]
fn embedded_subtitles_are_used() {
    for (codec, ext) in [("mov_text", "mp4"), ("srt", "mkv")] {
        let dir = tempfile::tempdir().unwrap();
        let src = mux(dir.path(), codec, ext);
        let out = dir.path().join("o.gba");
        let res = convert(&[src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
        assert!(res.status.success(), "{}", String::from_utf8_lossy(&res.stderr));
        assert_eq!(subtitle_count(&out), 2, "{codec}");
        let out2 = dir.path().join("n.gba");
        assert!(convert(&[src.to_str().unwrap(), "--no-subtitles", "-o", out2.to_str().unwrap()]).status.success());
        assert_eq!(subtitle_count(&out2), 0);
    }
}

#[test]
fn rejects_bad_subtitle_files() {
    let dir = tempfile::tempdir().unwrap();
    let clip = make_clip(dir.path(), true, 2);
    let bad = write(dir.path(), "b.srt", "1\n00:00:01 --> 00:00:02,000\nx\n");
    let res = convert(&[clip.to_str().unwrap(), "--subtitles", bad.to_str().unwrap()]);
    assert!(!res.status.success());
    assert!(String::from_utf8_lossy(&res.stderr).contains("line 2"));
}

#[test]
fn rejects_non_utf8_subtitles() {
    let dir = tempfile::tempdir().unwrap();
    let clip = make_clip(dir.path(), true, 2);
    let p = dir.path().join("w.srt");
    std::fs::write(&p, b"1\n00:00:01,000 --> 00:00:02,000\ncaf\xe9\n").unwrap();
    let res = convert(&[clip.to_str().unwrap(), "--subtitles", p.to_str().unwrap()]);
    assert!(!res.status.success());
    assert!(String::from_utf8_lossy(&res.stderr).contains("UTF-8"));
}

#[test]
fn reports_missing_glyphs() {
    let dir = tempfile::tempdir().unwrap();
    let clip = make_clip(dir.path(), true, 3);
    let srt = write(dir.path(), "j.srt", "1\n00:00:01,000 --> 00:00:02,000\nこんにちは\n");
    let out = dir.path().join("o.gba");
    let res = convert(&[clip.to_str().unwrap(), "--subtitles", srt.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert!(res.status.success());
    assert!(String::from_utf8_lossy(&res.stderr).contains("characters not in the font"));
}

#[test]
fn unusable_embedded_subtitles_are_skipped_with_a_notice() {
    let dir = tempfile::tempdir().unwrap();
    let clip = make_clip(dir.path(), true, 3);
    let srt = write(dir.path(), "styling.srt", "1\n00:00:01,000 --> 00:00:02,000\n{\\an8}\n");
    let src = dir.path().join("styled.mkv");
    let ok = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i"])
        .arg(&clip)
        .arg("-i")
        .arg(&srt)
        .args(["-map", "0", "-map", "1", "-c:v", "copy", "-c:a", "copy", "-c:s", "srt"])
        .arg(&src)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    let out = dir.path().join("o.gba");
    let res = convert(&[src.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    let err = String::from_utf8_lossy(&res.stderr);
    assert!(res.status.success(), "{err}");
    assert!(err.contains("embedded subtitles skipped"), "{err}");
    assert_eq!(subtitle_count(&out), 0);
}

#[test]
fn multiple_inputs_make_a_menu_cartridge() {
    let dir = tempfile::tempdir().unwrap();
    let a = common::make_source(dir.path(), "first", "testsrc=size=320x180:rate=30", 2);
    let b = common::make_source(dir.path(), "second", "smptebars=size=640x480:rate=30", 3);
    let out = dir.path().join("set.gba");
    let res = convert(&[a.to_str().unwrap(), b.to_str().unwrap(), "--title", "My set", "-o", out.to_str().unwrap()]);
    assert!(res.status.success(), "{}", String::from_utf8_lossy(&res.stderr));
    let bytes = std::fs::read(&out).unwrap();
    let c = parse(container_of(&bytes).unwrap()).unwrap();
    assert_eq!(c.video_count, 2);
    assert_eq!(c.title, "My set");
    assert_eq!(c.videos[0].layout, Layout::fit(320, 180, false));
    assert_eq!(c.videos[1].layout, Layout::fit(640, 480, false));
    assert!((38..=42).contains(&c.videos[0].frames.len()));
    assert!((58..=62).contains(&c.videos[1].frames.len()));
    assert_eq!(c.menu_pages.len(), 1);
}

#[test]
fn trimming_flags_need_a_single_input() {
    let dir = tempfile::tempdir().unwrap();
    let a = common::make_source(dir.path(), "a", "testsrc=size=320x180:rate=30", 1);
    let b = common::make_source(dir.path(), "b", "testsrc=size=320x180:rate=30", 1);
    let res = convert(&[a.to_str().unwrap(), b.to_str().unwrap(), "--start", "0:00.5"]);
    assert!(!res.status.success());
    assert!(String::from_utf8_lossy(&res.stderr).contains("single input"));
}

#[test]
fn shared_budget_falls_through() {
    let dir = tempfile::tempdir().unwrap();
    let a = common::make_source(dir.path(), "a", "testsrc=size=320x180:rate=30", 2);
    let b = common::make_source(dir.path(), "b", "testsrc2=size=320x180:rate=30", 6);
    let out = dir.path().join("set.gba");
    let probe = convert(&[a.to_str().unwrap(), b.to_str().unwrap(), "-o", out.to_str().unwrap()]);
    assert!(probe.status.success());
    let full = std::fs::metadata(&out).unwrap().len() as usize;
    let res = convert(&[a.to_str().unwrap(), b.to_str().unwrap(), "--max-size", &(full - 2000).to_string(), "-o", out.to_str().unwrap()]);
    let err = String::from_utf8_lossy(&res.stderr);
    assert!(res.status.success(), "{err}");
    assert!(err.contains("over budget"), "{err}");
    let bytes = std::fs::read(&out).unwrap();
    let c = parse(container_of(&bytes).unwrap()).unwrap();
    assert_eq!(c.video_count, 2);
    assert!(bytes.len() <= full - 2000);
    assert!(!c.videos[1].frames.is_empty());
}
