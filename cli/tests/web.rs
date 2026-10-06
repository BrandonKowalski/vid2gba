mod common;

use common::{find, run, screens};
use vid2gba_core::decode::Decoder;
use std::path::PathBuf;
use std::process::Command;
use vid2gba_core::container::parse;
use vid2gba_core::frame::Layout;
use vid2gba_core::rom::container_of;

fn reference_frame(src: &std::path::Path, t: f64, l: Layout) -> Vec<u8> {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-ss", &t.to_string(), "-i"])
        .arg(src)
        .args(["-frames:v", "1", "-vf", &format!("scale={}:{}:flags=area", l.w, l.h), "-pix_fmt", "rgb24", "-f", "rawvideo", "-"])
        .output()
        .unwrap();
    assert!(out.status.success());
    out.stdout
}

const DETAIL_MIN_DB: f64 = 24.0;

fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let mse: f64 = a.iter().zip(b).map(|(&x, &y)| (x as f64 - y as f64).powi(2)).sum::<f64>() / a.len() as f64;
    10.0 * (255.0f64 * 255.0 / mse.max(1e-9)).log10()
}

#[test]
#[ignore]
fn web_app_produces_playable_roms() {
    let out = tempfile::tempdir().unwrap();
    let app = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../web/app");
    let status = Command::new("npx").args(["playwright", "test"]).current_dir(&app).env("E2E_OUT", out.path()).status().unwrap();
    assert!(status.success(), "playwright failed");
    for (name, silent) in [("clip.gba", false), ("silent.gba", true), ("trimmed.gba", false)] {
        let path = out.path().join(name);
        let rom = std::fs::read(&path).unwrap_or_else(|_| panic!("{name} missing"));
        let c = parse(container_of(&rom).unwrap()).unwrap();
        assert_eq!(c.layout, Layout::fit(320, 180, false), "{name}");
        let pcm = vid2gba_core::adpcm::decode(c.audio, c.audio_samples as usize);
        assert_eq!(pcm.iter().all(|&s| s == 0), silent, "{name} audio");
        let m = screens(c.layout, &c.frames);
        let dir = tempfile::tempdir().unwrap();
        let shots = run(&path, dir.path(), &["start@10", "start@30", "60", "90"]);
        let a = find(&m, &shots[0], None).unwrap_or_else(|| panic!("{name} shot1"));
        let b = find(&m, &shots[1], None).unwrap_or_else(|| panic!("{name} shot2"));
        assert!(b > a, "{name}");
    }
    let bytes = std::fs::read(out.path().join("trimmed.gba")).unwrap();
    let trimmed = parse(container_of(&bytes).unwrap()).unwrap();
    assert!((38..=42).contains(&trimmed.frames.len()), "{}", trimmed.frames.len());
    let bytes = std::fs::read(out.path().join("clip.gba")).unwrap();
    let clip = parse(container_of(&bytes).unwrap()).unwrap();
    let mut d = Decoder::new(clip.layout);
    for f in &clip.frames[..=20] {
        d.decode(f);
    }
    let reference = reference_frame(&app.join("tests/fixtures/clip.webm"), 1.0, clip.layout);
    let q = psnr(&d.active_rgb(), &reference);
    eprintln!("web frame 20 vs source: {q:.1} dB");
    assert!(q > 18.0, "web output differs from the source video: {q:.1} dB");

    let late = std::fs::read(out.path().join("late-video.gba")).unwrap();
    let late = parse(container_of(&late).unwrap()).unwrap();
    assert!((118..=122).contains(&late.frames.len()), "late video frames {}", late.frames.len());
    let mut d = Decoder::new(late.layout);
    for f in &late.frames[..=40] {
        d.decode(f);
    }
    let src = app.join("tests/fixtures/late-video.webm");
    let on_time = psnr(&d.active_rgb(), &reference_frame(&src, 2.0, late.layout));
    let ahead = psnr(&d.active_rgb(), &reference_frame(&src, 2.5, late.layout));
    eprintln!("late video frame 40: {on_time:.1} dB at 2.0s, {ahead:.1} dB at 2.5s");
    assert!(on_time > ahead + 3.0, "video out of sync: {on_time:.1} dB at 2.0s vs {ahead:.1} dB at 2.5s");

    let late = std::fs::read(out.path().join("late-audio.gba")).unwrap();
    let late = parse(container_of(&late).unwrap()).unwrap();
    let pcm = vid2gba_core::adpcm::decode(late.audio, late.audio_samples as usize);
    let rms = |s: &[i16]| (s.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / s.len().max(1) as f64).sqrt();
    let lead = rms(&pcm[..(0.9 * 13379.0) as usize]);
    let body = rms(&pcm[(1.5 * 13379.0) as usize..(3.0 * 13379.0) as usize]);
    eprintln!("late audio: lead rms {lead:.0}, body rms {body:.0}, samples {}", pcm.len());
    assert!(lead < body / 10.0, "audio plays early: lead rms {lead:.0} vs body {body:.0}");
    assert!((pcm.len() as i64 - 6 * 13379).abs() < 700, "audio length {}", pcm.len());

    let detail = std::fs::read(out.path().join("detail.gba")).unwrap();
    let detail = parse(container_of(&detail).unwrap()).unwrap();
    let mut d = Decoder::new(detail.layout);
    for f in &detail.frames[..=10] {
        d.decode(f);
    }
    let q = psnr(&d.active_rgb(), &reference_frame(&app.join("tests/fixtures/detail.webm"), 0.5, detail.layout));
    eprintln!("detail frame 10 vs area-scaled source: {q:.1} dB");
    assert!(q > DETAIL_MIN_DB, "downscale quality {q:.1} dB");

    let subs = std::fs::read(out.path().join("subs.gba")).unwrap();
    let subs = parse(container_of(&subs).unwrap()).unwrap();
    assert_eq!(subs.subtitles.len(), 2);
    assert!(subs.subtitles.iter().all(|c| !c.sprites.is_empty() && c.w > 20));

    let set = std::fs::read(out.path().join("set.gba")).unwrap();
    let set = parse(container_of(&set).unwrap()).unwrap();
    assert_eq!(set.video_count, 2);
    assert_eq!(set.title, "Set");
    assert_eq!(set.menu_pages.len(), 1);
}
