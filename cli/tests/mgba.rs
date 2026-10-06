mod common;

use common::{convert, find, make_clip, run, same_rows, screens};
use std::path::{Path, PathBuf};
use vid2gba_core::container::{Parsed, ascii_title, parse};
use vid2gba_core::decode::Decoder;
use vid2gba_core::palette::expand;
use vid2gba_core::rom::container_of;
use vid2gba_core::{splash, title};

const OVERLAY_ROWS: std::ops::Range<usize> = 150..158;

fn build(dir: &Path, seconds: u32, extra: &[&str]) -> (PathBuf, Vec<u8>) {
    let clip = make_clip(dir, true, seconds);
    let rom_path = dir.join("e2e.gba");
    let mut args = vec![clip.to_str().unwrap(), "-o", rom_path.to_str().unwrap()];
    args.extend_from_slice(extra);
    let res = convert(&args);
    assert!(res.status.success(), "{}", String::from_utf8_lossy(&res.stderr));
    let rom = std::fs::read(&rom_path).unwrap();
    (rom_path, rom)
}

fn movie(c: &Parsed) -> Vec<Vec<u8>> {
    screens(c.layout, &c.frames)
}

fn title_rgb(c: &Parsed) -> Vec<u8> {
    let (img, pal) = title::render(&ascii_title(&c.title));
    img.iter().flat_map(|&i| expand(pal[i as usize])).collect()
}

#[test]
#[ignore]
fn boot_plays_splash_then_title() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build(dir.path(), 2, &[]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let s = screens(splash::LAYOUT, &c.splash_frames);
    let shots = run(&rom_path, dir.path(), &["30", "100", "260"]);
    let a = find(&s, &shots[0], None).expect("shot1 not a splash frame");
    let b = find(&s, &shots[1], None).expect("shot2 not a splash frame");
    assert!(b > a);
    assert!(same_rows(&shots[2], &title_rgb(&c), None), "title screen not shown after splash");
}

#[test]
#[ignore]
fn start_skips_splash() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build(dir.path(), 2, &[]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let shots = run(&rom_path, dir.path(), &["start@10", "40"]);
    assert!(same_rows(&shots[0], &title_rgb(&c), None));
}

fn check_playback(extra: &[&str]) {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build(dir.path(), 6, extra);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let m = movie(&c);
    let shots = run(&rom_path, dir.path(), &["start@10", "start@30", "120", "180", "240"]);
    let mut prev = None;
    for (i, shot) in shots.iter().enumerate() {
        let k = find(&m, shot, None).unwrap_or_else(|| panic!("shot{} matches no decoded frame", i + 1));
        if let Some(p) = prev {
            assert!(k > p, "playback did not advance: {p} then {k}");
        }
        prev = Some(k);
    }
}

#[test]
#[ignore]
fn plays_in_mgba() {
    check_playback(&[]);
}

#[test]
#[ignore]
fn plays_half_res_in_mgba() {
    check_playback(&["--half-res"]);
}

#[test]
#[ignore]
fn seek_moves_forward_and_back() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build(dir.path(), 20, &[]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let m = movie(&c);
    let shots = run(&rom_path, dir.path(), &["start@10", "start@30", "150", "right@160", "175", "left@200", "215"]);
    let k1 = find(&m, &shots[0], None).expect("shot1");
    let k2 = find(&m, &shots[1], Some(OVERLAY_ROWS)).expect("shot2");
    let k3 = find(&m, &shots[2], Some(OVERLAY_ROWS)).expect("shot3");
    assert!(k2 >= k1 + 4 * c.fps as usize, "forward seek {k1} -> {k2}");
    assert!(k3 < k2, "back seek {k2} -> {k3}");
    assert!(!same_rows(&shots[1], &m[k2], None), "overlay not visible after seek");
}

#[test]
#[ignore]
fn seek_back_at_start_stays_at_start() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build(dir.path(), 6, &[]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let m = movie(&c);
    let shots = run(&rom_path, dir.path(), &["start@10", "start@30", "left@40", "60"]);
    let k = find(&m, &shots[0], Some(OVERLAY_ROWS)).expect("shot");
    assert!(k < 2 * c.fps as usize, "{k}");
}

#[test]
#[ignore]
fn seek_past_end_returns_to_title() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build(dir.path(), 3, &[]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let shots = run(&rom_path, dir.path(), &["start@10", "start@30", "right@40", "120"]);
    assert!(same_rows(&shots[0], &title_rgb(&c), None));
}

#[test]
#[ignore]
fn pause_then_seek_past_end_returns_to_title() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build(dir.path(), 3, &[]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    for pause_at in [41, 45, 52] {
        let a = format!("a@{pause_at}");
        let shots = run(&rom_path, dir.path(), &["start@10", "start@30", &a, "right@70", "300"]);
        assert!(same_rows(&shots[0], &title_rgb(&c), None), "stuck after pausing at {pause_at}");
    }
}

#[test]
#[ignore]
fn pause_shows_overlay_then_hides_after_resume() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build(dir.path(), 10, &[]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let m = movie(&c);
    let shots = run(&rom_path, dir.path(), &["start@10", "start@30", "a@120", "140", "200", "a@220", "400"]);
    let k = find(&m, &shots[0], Some(OVERLAY_ROWS)).expect("paused frame");
    assert!(!same_rows(&shots[0], &m[k], None), "overlay hidden while paused");
    assert!(same_rows(&shots[1], &shots[0], None), "picture changed while paused");
    assert!(find(&m, &shots[2], None).is_some(), "overlay still visible 3 s after resume");
}

#[test]
#[ignore]
fn seek_while_paused_shows_target_frame() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build(dir.path(), 20, &[]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let m = movie(&c);
    let shots = run(&rom_path, dir.path(), &["start@10", "start@30", "a@100", "120", "right@130", "160", "220"]);
    let k1 = find(&m, &shots[0], Some(OVERLAY_ROWS)).expect("paused");
    let k2 = find(&m, &shots[1], Some(OVERLAY_ROWS)).expect("after seek");
    assert!(k2 >= k1 + 4 * c.fps as usize, "{k1} -> {k2}");
    assert!(same_rows(&shots[1], &shots[2], None), "advanced while paused");
}

fn srt(dir: &Path, start: &str, end: &str) -> String {
    let p = dir.join("t.srt");
    std::fs::write(&p, format!("1\n{start} --> {end}\nHELLO THERE\n")).unwrap();
    p.to_str().unwrap().to_string()
}

const SUB_ROWS: std::ops::Range<usize> = 110..158;

fn darken(rgb: &[u8]) -> Vec<u8> {
    rgb.iter()
        .map(|&v| {
            let c = v >> 3;
            let d = c - ((c as u16 * 10) >> 4) as u8;
            (d << 3) | (d >> 2)
        })
        .collect()
}

#[test]
#[ignore]
fn subtitles_show_and_hide() {
    let dir = tempfile::tempdir().unwrap();
    let s = srt(dir.path(), "00:00:02,000", "00:00:03,000");
    let (rom_path, rom) = build(dir.path(), 6, &["--subtitles", &s]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let m = movie(&c);
    let shots = run(&rom_path, dir.path(), &["start@10", "start@30", "121", "181", "241"]);
    assert!(find(&m, &shots[0], None).is_some(), "subtitle visible too early");
    let k = find(&m, &shots[1], Some(SUB_ROWS)).expect("frame under subtitle");
    assert!(!same_rows(&shots[1], &m[k], None), "subtitle not visible");
    assert!(find(&m, &shots[2], None).is_some(), "subtitle still visible");
}

#[test]
#[ignore]
fn cue_shows_after_seeking_into_it() {
    let dir = tempfile::tempdir().unwrap();
    let s = srt(dir.path(), "00:00:05,000", "00:00:15,000");
    let (rom_path, rom) = build(dir.path(), 20, &["--subtitles", &s]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let m = movie(&c);
    let shots = run(&rom_path, dir.path(), &["start@10", "start@30", "right@40", "60"]);
    let k = find(&m, &shots[0], Some(SUB_ROWS)).expect("frame");
    assert!(k >= 4 * c.fps as usize);
    assert!(!same_rows(&shots[0], &m[k], Some(OVERLAY_ROWS)), "subtitle missing after seek");
}

#[test]
#[ignore]
fn menu_toggles_subtitles() {
    let dir = tempfile::tempdir().unwrap();
    let s = srt(dir.path(), "00:00:01,000", "00:00:05,000");
    let (rom_path, rom) = build(dir.path(), 8, &["--subtitles", &s]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let m = movie(&c);
    let shots = run(&rom_path, dir.path(), &[
        "start@10", "start@30", "select@120", "130", "down@140", "a@150", "select@170", "200", "select@220", "down@230", "a@240", "select@250", "280",
    ]);
    let top = 40 * 240 * 3;
    let dark: Vec<Vec<u8>> = m.iter().map(|f| darken(f)).collect();
    assert!(
        dark.iter().any(|d| d[..top].iter().zip(&shots[0][..top]).all(|(a, b)| ((a >> 3) as i32 - (b >> 3) as i32).abs() <= 1)),
        "picture not darkened under menu"
    );
    assert!(find(&m, &shots[1], None).is_some(), "subtitle still shown after turning it off");
    let k = find(&m, &shots[2], Some(SUB_ROWS)).expect("frame");
    assert!(!same_rows(&shots[2], &m[k], None), "subtitle not shown after turning it on");
}

#[test]
#[ignore]
fn l_and_r_switch_picture() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build(dir.path(), 8, &[]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let mut d = Decoder::new(c.layout);
    let filled: Vec<Vec<u8>> = c
        .frames
        .iter()
        .map(|f| {
            d.decode(f);
            d.screen_rgb_affine(c.fill_pa as i32, c.fill_x, c.fill_y)
        })
        .collect();
    let m = movie(&c);
    let shots = run(&rom_path, dir.path(), &["start@10", "start@30", "l@100", "130", "r@160", "320"]);
    assert!(find(&filled, &shots[0], Some(OVERLAY_ROWS)).is_some(), "fill mode not applied");
    assert!(find(&m, &shots[1], None).is_some(), "original mode not restored");
}

#[test]
#[ignore]
fn menu_restart_goes_back_to_start() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build(dir.path(), 10, &[]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let m = movie(&c);
    let shots = run(&rom_path, dir.path(), &["start@10", "start@30", "300", "select@310", "down@320", "down@330", "a@340", "360"]);
    let before = find(&m, &shots[0], None).expect("before");
    let after = find(&m, &shots[1], None).expect("after");
    assert!(after < before && after < 20, "{before} -> {after}");
}


#[test]
#[ignore]
fn menu_picture_toggle_applies_immediately() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build(dir.path(), 8, &[]);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let mut d = Decoder::new(c.layout);
    let filled: Vec<Vec<u8>> = c
        .frames
        .iter()
        .map(|f| {
            d.decode(f);
            darken(&d.screen_rgb_affine(c.fill_pa as i32, c.fill_x, c.fill_y))
        })
        .collect();
    let shots = run(&rom_path, dir.path(), &["start@10", "start@30", "select@100", "down@110", "a@120", "140"]);
    let top = 40 * 240 * 3;
    assert!(
        filled.iter().any(|f| f[..top].iter().zip(&shots[0][..top]).all(|(a, b)| ((a >> 3) as i32 - (b >> 3) as i32).abs() <= 1)),
        "fill not applied behind the menu"
    );
}

#[test]
#[ignore]
fn progress_bar_is_centred() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, _) = build(dir.path(), 12, &[]);
    let shots = run(&rom_path, dir.path(), &["start@10", "start@30", "right@100", "130", "a@200", "230"]);
    for (name, shot) in [("playing", &shots[0]), ("paused", &shots[1])] {
        let row = &shot[153 * 240 * 3..154 * 240 * 3];
        let ink: Vec<usize> = (0..240).filter(|&x| row[x * 3..x * 3 + 3].iter().any(|&v| v > 40)).collect();
        let (left, right) = (ink[0], 239 - ink[ink.len() - 1]);
        assert!(left.abs_diff(right) <= 8, "{name}: margins {left} vs {right}");
    }
}

fn build_set(dir: &Path, sources: &[(&str, &str, u32)]) -> (PathBuf, Vec<u8>) {
    let paths: Vec<PathBuf> = sources.iter().map(|(n, l, s)| common::make_source(dir, n, l, *s)).collect();
    let rom_path = dir.join("set.gba");
    let mut args: Vec<&str> = paths.iter().map(|p| p.to_str().unwrap()).collect();
    args.extend(["--title", "Test set", "-o", rom_path.to_str().unwrap()]);
    let res = convert(&args);
    assert!(res.status.success(), "{}", String::from_utf8_lossy(&res.stderr));
    let rom = std::fs::read(&rom_path).unwrap();
    (rom_path, rom)
}

fn menu_rgb(c: &Parsed, page: usize, row: usize) -> Vec<u8> {
    let (pal_bytes, img) = c.menu_pages[page];
    let mut pal: Vec<u16> = pal_bytes.chunks(2).map(|b| u16::from_le_bytes([b[0], b[1]])).collect();
    let (normal, highlight) = (pal[5], pal[8]);
    for r in 0..3 {
        pal[5 + r] = if r == row { highlight } else { normal };
    }
    img.iter().flat_map(|&i| expand(pal[i as usize])).collect()
}

fn video_screens(c: &Parsed, i: usize) -> Vec<Vec<u8>> {
    screens(c.videos[i].layout, &c.videos[i].frames)
}

const THREE: [(&str, &str, u32); 3] = [
    ("alpha", "testsrc=size=320x180:rate=30", 2),
    ("beta", "testsrc=size=640x480:rate=30", 4),
    ("gamma", "testsrc2=size=320x240:rate=30", 3),
];

#[test]
#[ignore]
fn boots_to_main_menu() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build_set(dir.path(), &THREE);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let shots = run(&rom_path, dir.path(), &["260", "down@270", "290"]);
    assert!(same_rows(&shots[0], &menu_rgb(&c, 0, 0), None), "menu page not shown with first entry highlighted");
    assert!(same_rows(&shots[1], &menu_rgb(&c, 0, 1), None), "highlight did not move");
}

#[test]
#[ignore]
fn plays_selected_video() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build_set(dir.path(), &THREE);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let v1 = video_screens(&c, 1);
    let v0 = video_screens(&c, 0);
    let shots = run(&rom_path, dir.path(), &["down@200", "a@220", "300", "360"]);
    let a = find(&v1, &shots[0], None).expect("not playing video 2");
    let b = find(&v1, &shots[1], None).expect("video 2 stopped");
    assert!(b > a);
    assert!(find(&v0, &shots[0], None).is_none());
}

#[test]
#[ignore]
fn end_of_video_returns_to_menu_on_next_entry() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build_set(dir.path(), &THREE);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let shots = run(&rom_path, dir.path(), &["a@200", "400"]);
    assert!(same_rows(&shots[0], &menu_rgb(&c, 0, 1), None), "not back on the menu with entry 2 highlighted");
}

#[test]
#[ignore]
fn in_video_main_menu_returns_to_list() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build_set(dir.path(), &THREE);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let shots = run(&rom_path, dir.path(), &["down@200", "a@220", "a@260", "l@270", "select@300", "down@310", "down@320", "a@330", "360"]);
    assert!(same_rows(&shots[0], &menu_rgb(&c, 0, 1), None), "menu not restored cleanly");
}

#[test]
#[ignore]
fn menu_pages_and_wraps() {
    let dir = tempfile::tempdir().unwrap();
    let four = [THREE[0], THREE[1], THREE[2], ("delta", "testsrc=size=320x180:rate=30", 1)];
    let (rom_path, rom) = build_set(dir.path(), &four);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    assert_eq!(c.menu_pages.len(), 2);
    let shots = run(&rom_path, dir.path(), &["down@200", "down@210", "down@220", "240", "down@250", "270", "up@280", "300"]);
    assert!(same_rows(&shots[0], &menu_rgb(&c, 1, 0), None), "page 2 not shown");
    assert!(same_rows(&shots[1], &menu_rgb(&c, 0, 0), None), "did not wrap to the first entry");
    assert!(same_rows(&shots[2], &menu_rgb(&c, 1, 0), None), "did not wrap back to the last entry");
}

fn near_dark(frames: &[Vec<u8>], shot: &[u8], skip: &[std::ops::Range<usize>]) -> bool {
    frames.iter().any(|f| {
        darken(f).chunks(240 * 3).zip(shot.chunks(240 * 3)).enumerate().all(|(y, (a, b))| {
            skip.iter().any(|s| s.contains(&y)) || a.iter().zip(b).all(|(p, q)| ((p >> 3) as i32 - (q >> 3) as i32).abs() <= 1)
        })
    })
}

#[test]
#[ignore]
fn in_video_menu_shows_all_five_items() {
    let dir = tempfile::tempdir().unwrap();
    let plain = common::make_source(dir.path(), "plain", "testsrc=size=320x180:rate=30", 6);
    let s = srt(dir.path(), "00:00:05,000", "00:00:06,000");
    let subbed = dir.path().join("subbed.mkv");
    let ok = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-i", plain.to_str().unwrap(), "-i", &s, "-map", "0", "-map", "1", "-c", "copy", "-c:s", "srt"])
        .arg(&subbed)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    let other = common::make_source(dir.path(), "other", "testsrc2=size=320x240:rate=30", 2);
    let rom_path = dir.path().join("set.gba");
    let res = convert(&[subbed.to_str().unwrap(), other.to_str().unwrap(), "-o", rom_path.to_str().unwrap()]);
    assert!(res.status.success(), "{}", String::from_utf8_lossy(&res.stderr));
    let rom = std::fs::read(&rom_path).unwrap();
    let c = parse(container_of(&rom).unwrap()).unwrap();
    assert_eq!(c.videos[0].subtitles.len(), 1);
    let v0 = video_screens(&c, 0);
    let shots = run(&rom_path, dir.path(), &["a@220", "a@260", "select@300", "330"]);
    assert!(near_dark(&v0, &shots[0], &[48..110, 140..160]), "paused frame not found under the menu");
    assert!(!near_dark(&v0, &shots[0], &[48..97, 140..160]), "fifth menu item not drawn");
}

#[test]
#[ignore]
fn menu_appears_without_partial_frames() {
    const FROM: usize = 300;
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, rom) = build_set(dir.path(), &THREE);
    let c = parse(container_of(&rom).unwrap()).unwrap();
    let v0 = video_screens(&c, 0);
    let menu = menu_rgb(&c, 0, 1);
    let frames: Vec<String> = (FROM..FROM + 60).map(|f| f.to_string()).collect();
    let mut spec = vec!["a@200".to_string()];
    spec.extend(frames);
    let spec: Vec<&str> = spec.iter().map(|s| s.as_str()).collect();
    let shots = run(&rom_path, dir.path(), &spec);
    for (i, s) in shots.iter().enumerate() {
        let black = s.iter().all(|&v| v < 8);
        let ok = black || same_rows(s, &menu, None) || find(&v0, s, Some(OVERLAY_ROWS)).is_some();
        assert!(ok, "frame {} is neither video, black nor the menu", FROM + i);
    }
    assert!(same_rows(&shots[shots.len() - 1], &menu, None), "menu not shown");
}

#[test]
#[ignore]
fn pause_icon_has_room_before_the_time() {
    let dir = tempfile::tempdir().unwrap();
    let (rom_path, _) = build(dir.path(), 8, &[]);
    let shots = run(&rom_path, dir.path(), &["start@10", "start@30", "start@120", "200"]);
    let white: Vec<usize> = (0..240)
        .filter(|&x| OVERLAY_ROWS.clone().any(|y| shots[0][(y * 240 + x) * 3..(y * 240 + x) * 3 + 3].iter().all(|&v| v > 200)))
        .collect();
    let gaps: Vec<usize> = white.windows(2).map(|w| w[1] - w[0] - 1).filter(|&g| g > 0).collect();
    assert_eq!(gaps[0], 1, "pause icon bars");
    assert!(gaps[1] >= 4, "pause icon is {}px from the time", gaps[1]);
}
