use vid2gba_wasm::*;
use wasm_bindgen_test::*;

#[wasm_bindgen_test]
fn parses_and_validates_times() {
    assert_eq!(parse_time("1:30").unwrap(), 90.0);
    assert!(parse_time("x").is_err());
    assert!(validate_range(Some(10.0), None, 5.0).is_err());
    assert!(validate_range(None, None, 5.0).is_ok());
}

#[wasm_bindgen_test]
fn fits_layout_and_ladder() {
    let l = fit_layout(1920, 1080, false);
    assert_eq!((l.w, l.h, l.x_off, l.y_off, l.half_res), (240, 134, 0, 13, false));
    assert_eq!(candidates(None, false).len(), 7);
    assert!(candidates(Some(24), true).iter().all(|s| s.fps == 24 && s.half_res));
}

fn rgba_frame(w: u32, h: u32, shift: u32) -> Vec<u8> {
    (0..w * h).flat_map(|i| [((i + shift) % 256) as u8, (i / w % 256) as u8, 40, 255]).collect()
}

fn packed_gop(l: &JsLayout, frames: u32) -> Vec<Vec<u8>> {
    let mut s = Splitter::new(2, l.w, l.h, false);
    let mut gops = Vec::new();
    for i in 0..frames {
        gops.extend(s.push(&rgba_frame(l.w, l.h, i * 3)));
    }
    gops.extend(s.finish());
    gops.iter().map(|g| encode_gop_packed(g, l.w, l.h, 64, 600)).collect()
}

#[wasm_bindgen_test]
fn builds_a_parseable_rom() {
    let l = fit_layout(16, 16, false);
    let mut enc = AudioEncoder::new(48000.0);
    enc.push(&vec![0.1; 48000]);
    let audio = enc.finish();
    assert!((enc.samples() as i64 - 13379).abs() <= 2);
    let mut cart = CartBuilder::new();
    cart.add_video(16, 16, &audio, enc.samples(), &Subtitles::empty());
    let fixed = cart.fixed_size("Test").unwrap();
    cart.begin_step(32 * 1024 * 1024 - fixed);
    let gops = packed_gop(&l, 10);
    assert_eq!(gops.len(), 3);
    for g in gops {
        assert!(cart.accept(&g));
    }
    cart.end_video(&l, 2);
    assert_eq!(cart.frames_fit(), 10);
    let rom = cart.build("Test").unwrap();
    assert!(fixed < rom.len() as u32);
    let c = vid2gba_core::container::parse(vid2gba_core::rom::container_of(&rom).unwrap()).unwrap();
    assert_eq!(c.video_count, 1);
    assert_eq!(c.frames.len(), 10);
    assert_eq!(c.title, "Test");
}

#[wasm_bindgen_test]
fn builds_a_two_video_set_with_menu() {
    let a = fit_layout(16, 16, false);
    let b = fit_layout(32, 16, false);
    let mut cart = CartBuilder::new();
    cart.add_video(16, 16, &[], 0, &Subtitles::empty());
    cart.add_video(32, 16, &[], 0, &Subtitles::empty());
    cart.add_menu_entry("A", 1.0, &vec![200; 64 * 40 * 4]);
    cart.add_menu_entry("B", 2.0, &vec![50; 64 * 40 * 4]);
    let fixed = cart.fixed_size("Set").unwrap();
    cart.begin_step(32 * 1024 * 1024 - fixed);
    for g in packed_gop(&a, 4) {
        assert!(cart.accept(&g));
    }
    cart.end_video(&a, 2);
    for g in packed_gop(&b, 6) {
        assert!(cart.accept(&g));
    }
    cart.end_video(&b, 2);
    let rom = cart.build("Set").unwrap();
    let c = vid2gba_core::container::parse(vid2gba_core::rom::container_of(&rom).unwrap()).unwrap();
    assert_eq!(c.video_count, 2);
    assert_eq!(c.videos[0].frames.len(), 4);
    assert_eq!(c.videos[1].frames.len(), 6);
    assert_eq!(c.videos[1].layout, vid2gba_core::frame::Layout::fit(32, 16, false));
    assert_eq!(c.menu_pages.len(), 1);
}

#[wasm_bindgen_test]
fn real_gba_target_brightens() {
    let l = fit_layout(16, 16, false);
    let frame = vec![100u8; (l.w * l.h * 4) as usize];
    let mut emu = Splitter::new(1, l.w, l.h, false);
    let mut gba = Splitter::new(1, l.w, l.h, true);
    emu.push(&frame);
    gba.push(&frame);
    let (e, g) = (emu.finish().unwrap(), gba.finish().unwrap());
    assert_eq!(e[0], 100);
    assert!(g[0] > 130);
}

#[wasm_bindgen_test]
fn budget_rejects_when_full() {
    let l = fit_layout(16, 16, false);
    let mut s = Splitter::new(1, l.w, l.h, false);
    s.push(&rgba_frame(l.w, l.h, 0));
    let packed = encode_gop_packed(&s.finish().unwrap(), l.w, l.h, 64, 600);
    let mut cart = CartBuilder::new();
    cart.add_video(16, 16, &[], 0, &Subtitles::empty());
    cart.begin_step(10);
    assert!(!cart.accept(&packed));
    assert_eq!(cart.frames_fit(), 0);
}

#[wasm_bindgen_test]
fn silence_and_messages() {
    let mut enc = AudioEncoder::new(13379.0);
    enc.push_silence(1000);
    enc.finish();
    assert_eq!(enc.samples(), 1000);
    assert!(too_big_message(100.0, 1000, 10.0).contains("Set a start and end"));
    assert!(audio_too_big_message(100.0, 1000, 2000, 1500, false).contains("audio alone"));
    assert_eq!(max_rom_size(), 32 * 1024 * 1024);
}

#[wasm_bindgen_test]
fn subtitles_parse_trim_and_build() {
    let mut s = Subtitles::new("1\n00:00:01,000 --> 00:00:02,000\nHi\n\n2\n00:00:05,000 --> 00:00:06,000\nBye\n", 0.5, 4.0).unwrap();
    assert_eq!(s.count(), 1);
    assert_eq!(s.text(0), "Hi");
    s.set_image(0, &vec![255; 20 * 10], 20, 10);
    let l = fit_layout(16, 16, false);
    let mut cart = CartBuilder::new();
    cart.add_video(16, 16, &[], 0, &s);
    cart.begin_step(32 * 1024 * 1024);
    cart.end_video(&l, 2);
    let rom = cart.build("T").unwrap();
    let c = vid2gba_core::container::parse(vid2gba_core::rom::container_of(&rom).unwrap()).unwrap();
    assert_eq!(c.subtitles.len(), 1);
    assert_eq!(c.subtitles[0].start_sample, (0.5f64 * 13379.0).round() as u32);
    assert!(Subtitles::new("garbage", 0.0, 1.0).is_err());
}
