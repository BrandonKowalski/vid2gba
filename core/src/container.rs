use crate::frame::Layout;
use anyhow::{Result, ensure};

pub const AUDIO_RATE: u32 = 13379;
pub const MAGIC: &[u8; 8] = b"VID2GBA\0";
pub const VERSION: u32 = 4;
const HEADER_LEN: usize = 160;

pub struct Video<'a> {
    pub layout: Layout,
    pub fps: u32,
    pub frames: &'a [Vec<u8>],
    pub audio: &'a [u8],
    pub audio_samples: u32,
    pub title: &'a str,
    pub subtitles: &'a [crate::subs::SubCue],
}

pub struct Parsed<'a> {
    pub layout: Layout,
    pub fps: u32,
    pub spf_q16: u32,
    pub audio_samples: u32,
    pub audio: &'a [u8],
    pub frames: Vec<&'a [u8]>,
    pub title: String,
    pub splash_frames: Vec<&'a [u8]>,
    pub splash_audio: &'a [u8],
    pub splash_audio_samples: u32,
    pub overlay: &'a [u8],
    pub overlay_tiles: u32,
    pub total_samples: u32,
    pub subtitles: Vec<ParsedCue<'a>>,
    pub subtitle_palette: &'a [u8],
    pub fill_pa: u16,
    pub fill_x: i32,
    pub fill_y: i32,
    pub menu: &'a [u8],
    pub menu_tiles: u32,
    pub video_count: u32,
    pub videos: Vec<ParsedVideo<'a>>,
    pub menu_pages: Vec<(&'a [u8], &'a [u8])>,
}

pub struct ParsedVideo<'a> {
    pub layout: Layout,
    pub fps: u32,
    pub spf_q16: u32,
    pub total_samples: u32,
    pub audio_samples: u32,
    pub audio: &'a [u8],
    pub frames: Vec<&'a [u8]>,
    pub subtitles: Vec<ParsedCue<'a>>,
    pub fill_pa: u16,
    pub fill_x: i32,
    pub fill_y: i32,
}

pub struct ParsedCue<'a> {
    pub start_sample: u32,
    pub end_sample: u32,
    pub w: u8,
    pub h: u8,
    pub sprites: Vec<(u8, u8, &'a [u8])>,
}

pub fn ascii_title(s: &str) -> String {
    s.trim().chars().map(|c| if (' '..='~').contains(&c) { c } else { '?' }).collect()
}

fn align4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

fn write_frames(out: &mut Vec<u8>, frames: &[Vec<u8>]) -> usize {
    align4(out);
    let table = out.len();
    out.resize(table + frames.len() * 4, 0);
    for (i, f) in frames.iter().enumerate() {
        align4(out);
        let off = out.len() as u32;
        out[table + i * 4..][..4].copy_from_slice(&off.to_le_bytes());
        out.extend_from_slice(f);
    }
    align4(out);
    table
}

fn to_samples(t: f64) -> u32 {
    (t * AUDIO_RATE as f64).round().max(0.0) as u32
}

fn write_subtitles(out: &mut Vec<u8>, subs: &[crate::subs::SubCue]) -> usize {
    align4(out);
    let off = out.len();
    for c in crate::subs::palette() {
        out.extend(c.to_le_bytes());
    }
    let table = out.len();
    out.resize(table + subs.len() * 16, 0);
    for (i, s) in subs.iter().enumerate() {
        align4(out);
        let data_off = out.len() as u32;
        for (x, y, tiles) in &s.image.sprites {
            out.extend([*x, *y, 0, 0]);
            out.extend_from_slice(tiles);
        }
        let e = &mut out[table + i * 16..][..16];
        e[0..4].copy_from_slice(&to_samples(s.start).to_le_bytes());
        e[4..8].copy_from_slice(&to_samples(s.end).to_le_bytes());
        e[8..12].copy_from_slice(&data_off.to_le_bytes());
        e[12] = s.image.sprites.len() as u8;
        e[13] = s.image.w;
        e[14] = s.image.h;
    }
    align4(out);
    off
}

struct Written {
    frames_off: u32,
    audio_off: u32,
    subtitle_off: u32,
    spf: u32,
    total: u32,
}

pub fn build(v: &Video) -> Vec<u8> {
    build_set(v.title, std::slice::from_ref(v), &[])
}

pub fn build_set(title: &str, videos: &[Video], pages: &[crate::dvdmenu::MenuPage]) -> Vec<u8> {
    let title = ascii_title(title);
    let (image, colors) = crate::title::render(&title);
    let splash = crate::splash::encoded();
    let mut out = vec![0u8; HEADER_LEN];
    let title_off = out.len();
    for c in colors {
        out.extend(c.to_le_bytes());
    }
    out.extend_from_slice(&image);
    align4(&mut out);
    let overlay_off = out.len();
    out.extend_from_slice(&crate::overlay::data());
    align4(&mut out);
    let menu_off = out.len();
    out.extend_from_slice(&crate::menu::data());
    align4(&mut out);
    let splash_audio_off = out.len();
    out.extend_from_slice(&splash.audio);
    let splash_frames_off = write_frames(&mut out, &splash.records);
    let written: Vec<Written> = videos
        .iter()
        .map(|v| {
            let subtitle_off = write_subtitles(&mut out, v.subtitles) as u32;
            let audio_off = out.len() as u32;
            out.extend_from_slice(v.audio);
            let frames_off = write_frames(&mut out, v.frames) as u32;
            let spf = (((AUDIO_RATE as u64) << 16) / v.fps as u64) as u32;
            let total = ((v.frames.len() as u64 * spf as u64) >> 16) as u32;
            Written { frames_off, audio_off, subtitle_off, spf, total }
        })
        .collect();
    let menu_pages_off = if pages.is_empty() {
        0
    } else {
        align4(&mut out);
        let off = out.len();
        out.extend((pages.len() as u32).to_le_bytes());
        for p in pages {
            for c in p.palette {
                out.extend(c.to_le_bytes());
            }
            out.extend_from_slice(&p.image);
        }
        off
    };
    align4(&mut out);
    let videos_off = out.len();
    for (v, w) in videos.iter().zip(&written) {
        let l = v.layout;
        let (fill_pa, fill_x, fill_y) = l.fill();
        for x in [v.frames.len() as u32, w.frames_off, w.audio_off, v.audio_samples, w.spf, w.total] {
            out.extend(x.to_le_bytes());
        }
        for x in [l.w, l.h, l.x_off, l.y_off] {
            out.extend((x as u16).to_le_bytes());
        }
        out.extend((l.half_res as u32).to_le_bytes());
        out.extend(fill_pa.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(fill_x.to_le_bytes());
        out.extend(fill_y.to_le_bytes());
        out.extend((v.subtitles.len() as u32).to_le_bytes());
        out.extend(w.subtitle_off.to_le_bytes());
        out.extend(v.fps.to_le_bytes());
        out.extend(0u32.to_le_bytes());
    }
    let v0 = &videos[0];
    let w0 = &written[0];
    let l = v0.layout;
    let (fill_pa, fill_x, fill_y) = l.fill();
    let mut h = Vec::with_capacity(HEADER_LEN);
    h.extend_from_slice(MAGIC);
    for x in [VERSION, l.half_res as u32, v0.fps, 1, v0.frames.len() as u32] {
        h.extend(x.to_le_bytes());
    }
    for x in [l.w, l.h, l.x_off, l.y_off] {
        h.extend((x as u16).to_le_bytes());
    }
    for x in [AUDIO_RATE, v0.audio_samples, w0.audio_off, w0.frames_off, w0.spf, title_off as u32] {
        h.extend(x.to_le_bytes());
    }
    let mut t = [0u8; 32];
    for (i, b) in title.bytes().take(31).enumerate() {
        t[i] = b;
    }
    h.extend_from_slice(&t);
    for x in [
        splash.records.len() as u32,
        splash_frames_off as u32,
        splash_audio_off as u32,
        splash.samples,
        overlay_off as u32,
        crate::overlay::TILE_COUNT,
        w0.total,
    ] {
        h.extend(x.to_le_bytes());
    }
    h.extend((v0.subtitles.len() as u32).to_le_bytes());
    h.extend(w0.subtitle_off.to_le_bytes());
    h.extend(fill_pa.to_le_bytes());
    h.extend(0u16.to_le_bytes());
    h.extend(fill_x.to_le_bytes());
    h.extend(fill_y.to_le_bytes());
    h.extend((menu_off as u32).to_le_bytes());
    h.extend(crate::menu::TILE_COUNT.to_le_bytes());
    h.extend((videos.len() as u32).to_le_bytes());
    h.extend((videos_off as u32).to_le_bytes());
    h.extend((menu_pages_off as u32).to_le_bytes());
    out[..h.len()].copy_from_slice(&h);
    out
}

fn u32_at(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}

fn u16_at(d: &[u8], o: usize) -> usize {
    u16::from_le_bytes([d[o], d[o + 1]]) as usize
}

fn parse_cues(d: &[u8], off: usize, count: usize) -> Result<Vec<ParsedCue<'_>>> {
    ensure!(off + 32 + count * 16 <= d.len(), "container subtitle offsets out of range");
    Ok((0..count)
        .map(|i| {
            let e = off + 32 + i * 16;
            let data_off = u32_at(d, e + 8) as usize;
            let n = d[e + 12] as usize;
            ParsedCue {
                start_sample: u32_at(d, e),
                end_sample: u32_at(d, e + 4),
                w: d[e + 13],
                h: d[e + 14],
                sprites: (0..n)
                    .map(|s| {
                        let p = data_off + s * 260;
                        (d[p], d[p + 1], &d[p + 4..p + 260])
                    })
                    .collect(),
            }
        })
        .collect())
}

pub fn parse(d: &[u8]) -> Result<Parsed<'_>> {
    ensure!(d.len() >= HEADER_LEN && &d[..8] == MAGIC, "not a vid2gba container");
    ensure!(u32_at(d, 8) == VERSION, "unsupported container version {}", u32_at(d, 8));
    let layout = Layout {
        half_res: u32_at(d, 12) & 1 != 0,
        w: u16_at(d, 28),
        h: u16_at(d, 30),
        x_off: u16_at(d, 32),
        y_off: u16_at(d, 34),
    };
    let count = u32_at(d, 24) as usize;
    let audio_off = u32_at(d, 44) as usize;
    let frames_off = u32_at(d, 48) as usize;
    ensure!(audio_off <= frames_off && frames_off + count * 4 <= d.len(), "container offsets out of range");
    let frames = (0..count).map(|i| &d[u32_at(d, frames_off + i * 4) as usize..]).collect();
    let splash_count = u32_at(d, 92) as usize;
    let splash_frames_off = u32_at(d, 96) as usize;
    let splash_audio_off = u32_at(d, 100) as usize;
    let overlay_off = u32_at(d, 108) as usize;
    let overlay_tiles = u32_at(d, 112);
    ensure!(
        splash_audio_off <= splash_frames_off && splash_frames_off + splash_count * 4 <= d.len(),
        "container splash offsets out of range"
    );
    let splash_frames = (0..splash_count).map(|i| &d[u32_at(d, splash_frames_off + i * 4) as usize..]).collect();
    let overlay = &d[overlay_off..overlay_off + 32 + overlay_tiles as usize * 32];
    let i32_at = |o: usize| i32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    let sub_count = u32_at(d, 120) as usize;
    let sub_off = u32_at(d, 124) as usize;
    let menu_off = u32_at(d, 140) as usize;
    let menu_tiles = u32_at(d, 144);
    ensure!(sub_off + 32 + sub_count * 16 <= d.len(), "container subtitle offsets out of range");
    let subtitles = parse_cues(d, sub_off, sub_count)?;
    let video_count = u32_at(d, 148);
    let videos_off = u32_at(d, 152) as usize;
    let menu_pages_off = u32_at(d, 156) as usize;
    ensure!(videos_off + video_count as usize * 64 <= d.len(), "container video table out of range");
    let mut videos = Vec::with_capacity(video_count as usize);
    for i in 0..video_count as usize {
        let e = videos_off + i * 64;
        let count = u32_at(d, e) as usize;
        let frames_off = u32_at(d, e + 4) as usize;
        let audio_off = u32_at(d, e + 8) as usize;
        ensure!(audio_off <= frames_off && frames_off + count * 4 <= d.len(), "container video offsets out of range");
        videos.push(ParsedVideo {
            layout: Layout {
                half_res: u32_at(d, e + 32) & 1 != 0,
                w: u16_at(d, e + 24),
                h: u16_at(d, e + 26),
                x_off: u16_at(d, e + 28),
                y_off: u16_at(d, e + 30),
            },
            fps: u32_at(d, e + 56),
            spf_q16: u32_at(d, e + 16),
            total_samples: u32_at(d, e + 20),
            audio_samples: u32_at(d, e + 12),
            audio: &d[audio_off..frames_off],
            frames: (0..count).map(|k| &d[u32_at(d, frames_off + k * 4) as usize..]).collect(),
            subtitles: parse_cues(d, u32_at(d, e + 52) as usize, u32_at(d, e + 48) as usize)?,
            fill_pa: u16::from_le_bytes([d[e + 36], d[e + 37]]),
            fill_x: i32_at(e + 40),
            fill_y: i32_at(e + 44),
        });
    }
    let mut menu_pages = Vec::new();
    if menu_pages_off != 0 {
        let n = u32_at(d, menu_pages_off) as usize;
        let page_bytes = 512 + 240 * 160;
        ensure!(menu_pages_off + 4 + n * page_bytes <= d.len(), "container menu pages out of range");
        for k in 0..n {
            let p = menu_pages_off + 4 + k * page_bytes;
            menu_pages.push((&d[p..p + 512], &d[p + 512..p + page_bytes]));
        }
    }
    let title: String = d[60..92].iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
    Ok(Parsed {
        layout,
        fps: u32_at(d, 16),
        spf_q16: u32_at(d, 52),
        audio_samples: u32_at(d, 40),
        audio: &d[audio_off..frames_off],
        frames,
        title,
        splash_frames,
        splash_audio: &d[splash_audio_off..splash_frames_off],
        splash_audio_samples: u32_at(d, 104),
        overlay,
        overlay_tiles,
        total_samples: u32_at(d, 116),
        subtitles,
        subtitle_palette: &d[sub_off..sub_off + 32],
        fill_pa: u16::from_le_bytes([d[128], d[129]]),
        fill_x: i32_at(132),
        fill_y: i32_at(136),
        menu: &d[menu_off..menu_off + 64 + menu_tiles as usize * 32],
        menu_tiles,
        video_count,
        videos,
        menu_pages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> Layout {
        Layout { half_res: true, w: 120, h: 66, x_off: 0, y_off: 7 }
    }

    #[test]
    fn roundtrip() {
        let frames = vec![vec![3, 0, 0, 0, 1, 2, 3, 4], vec![0, 0, 0, 0, 0x7F, 0, 0, 0], vec![1; 12]];
        let audio = vec![9u8; 1028];
        let data = build(&Video { layout: layout(), fps: 15, frames: &frames, audio: &audio, audio_samples: 2000, title: "My Clip", subtitles: &[] });
        assert_eq!(data.len() % 4, 0);
        let p = parse(&data).unwrap();
        assert_eq!(p.layout, layout());
        assert_eq!(p.fps, 15);
        assert_eq!(p.spf_q16, ((13379u64 << 16) / 15) as u32);
        assert_eq!(p.audio_samples, 2000);
        assert_eq!(&p.audio[..1028], &audio[..]);
        assert_eq!(p.title, "My Clip");
        assert_eq!(p.frames.len(), 3);
        for (got, want) in p.frames.iter().zip(&frames) {
            assert_eq!(&got[..want.len()], &want[..]);
            assert_eq!((got.as_ptr() as usize - data.as_ptr() as usize) % 4, 0);
        }
    }

    #[test]
    fn title_is_ascii_and_truncated() {
        assert_eq!(ascii_title("  Café ☕ Vlog "), "Caf? ? Vlog");
        let long = "x".repeat(50);
        let data = build(&Video { layout: layout(), fps: 10, frames: &[], audio: &[], audio_samples: 0, title: &long, subtitles: &[] });
        assert_eq!(parse(&data).unwrap().title, "x".repeat(31));
    }

    #[test]
    fn rejects_bad_magic() {
        assert!(parse(&[0u8; 200]).is_err());
        assert!(parse(&[0u8; 4]).is_err());
    }

    #[test]
    fn embeds_title_screen() {
        let data = build(&Video { layout: layout(), fps: 10, frames: &[], audio: &[], audio_samples: 0, title: "T", subtitles: &[] });
        let off = u32::from_le_bytes(data[56..60].try_into().unwrap()) as usize;
        let (img, pal) = crate::title::render("T");
        assert_eq!(&data[off..off + 2], &pal[0].to_le_bytes());
        assert_eq!(&data[off + 512..off + 512 + img.len()], &img[..]);
    }

    #[test]
    fn header_and_sections() {
        let frames = vec![vec![3, 0, 0, 0, 1, 2, 3, 4]; 30];
        let data = build(&Video { layout: layout(), fps: 15, frames: &frames, audio: &[7; 1028], audio_samples: 2000, title: "T", subtitles: &[] });
        assert_eq!(&data[..8], b"VID2GBA\0");
        assert_eq!(u32::from_le_bytes(data[8..12].try_into().unwrap()), VERSION);
        let p = parse(&data).unwrap();
        let splash = crate::splash::encoded();
        assert_eq!(p.splash_frames.len(), splash.records.len());
        assert_eq!(&p.splash_frames[5][..splash.records[5].len()], &splash.records[5][..]);
        assert_eq!(&p.splash_audio[..splash.audio.len()], &splash.audio[..]);
        assert_eq!(p.splash_audio_samples, splash.samples);
        assert_eq!(p.overlay, &crate::overlay::data()[..]);
        assert_eq!(p.overlay_tiles, crate::overlay::TILE_COUNT);
        assert_eq!(p.total_samples, ((30u64 * p.spf_q16 as u64) >> 16) as u32);
        assert_eq!(p.frames.len(), 30);
        for off in [92usize, 96, 100, 104, 108, 112, 116] {
            assert_ne!(u32::from_le_bytes(data[off..off + 4].try_into().unwrap()), 0, "field at {off}");
        }
    }

    #[test]
    fn rejects_v1_containers() {
        let mut data = build(&Video { layout: layout(), fps: 10, frames: &[], audio: &[], audio_samples: 0, title: "T", subtitles: &[] });
        data[..8].copy_from_slice(b"YT2GBV\0\0");
        assert!(parse(&data).is_err());
    }

    #[test]
    fn budget_charges_match_container_growth() {
        use crate::codec::{Budget, Params, encode_gop};
        use crate::frame::Frame;
        let frames: Vec<Frame> = (0..7)
            .map(|i| Frame { w: 30, h: 22, rgb: (0..30 * 22 * 3).map(|p| ((p * 7 + i * 13) % 251) as u8).collect() })
            .collect();
        let records = encode_gop(&frames, &Params { codebook: 37, skip: 0 });
        assert!(records.iter().all(|r| r.len() % 4 == 0));
        let mut budget = Budget::new(usize::MAX);
        assert!(budget.accept(records.clone()));
        let charged: usize = records.iter().map(|r| r.len() + 4).sum();
        let l = Layout { half_res: false, w: 30, h: 22, x_off: 0, y_off: 0 };
        let empty = build(&Video { layout: l, fps: 10, frames: &[], audio: &[], audio_samples: 0, title: "T", subtitles: &[] });
        let full = build(&Video { layout: l, fps: 10, frames: &records, audio: &[], audio_samples: 0, title: "T", subtitles: &[] });
        assert_eq!(full.len(), empty.len() + charged);
    }

    #[test]
    fn v3_round_trip_with_subtitles() {
        let image = crate::subs::pack(&vec![255u8; 40 * 12], 40, 12);
        let subs = vec![crate::subs::SubCue { start: 1.0, end: 2.5, image: image.clone() }];
        let l = Layout { half_res: false, w: 240, h: 134, x_off: 0, y_off: 13 };
        let data = build(&Video { layout: l, fps: 20, frames: &[], audio: &[], audio_samples: 0, title: "T", subtitles: &subs });
        assert_eq!(&data[..8], b"VID2GBA\0");
        assert_eq!(u32::from_le_bytes(data[8..12].try_into().unwrap()), VERSION);
        let p = parse(&data).unwrap();
        assert_eq!(p.video_count, 1);
        assert_eq!((p.fill_pa, p.fill_x, p.fill_y), l.fill());
        assert_eq!(p.menu, &crate::menu::data()[..]);
        assert_eq!(p.menu_tiles, crate::menu::TILE_COUNT);
        assert_eq!(p.subtitle_palette.len(), 32);
        assert_eq!(p.subtitles.len(), 1);
        let c = &p.subtitles[0];
        assert_eq!((c.start_sample, c.end_sample, c.w, c.h), (13379, 33448, 40, 12));
        assert_eq!(c.sprites.len(), image.sprites.len());
        assert_eq!(c.sprites[0].2, &image.sprites[0].2[..]);
    }

    #[test]
    fn rejects_v2_containers() {
        let mut data = build(&Video { layout: layout(), fps: 10, frames: &[], audio: &[], audio_samples: 0, title: "T", subtitles: &[] });
        data[8..12].copy_from_slice(&2u32.to_le_bytes());
        assert!(parse(&data).is_err());
        data[8..12].copy_from_slice(&3u32.to_le_bytes());
        assert!(parse(&data).is_err());
    }

    fn video<'a>(layout: Layout, frames: &'a [Vec<u8>], subs: &'a [crate::subs::SubCue]) -> Video<'a> {
        Video { layout, fps: 10, frames, audio: &[1; 1028], audio_samples: 1500, title: "ignored", subtitles: subs }
    }

    #[test]
    fn v4_round_trip_three_videos() {
        let wide = Layout::fit(1920, 1080, false);
        let tall = Layout::fit(1080, 1920, false);
        let half = Layout::fit(640, 480, true);
        let f1 = vec![vec![3, 0, 0, 0, 1, 1, 1, 1]; 5];
        let f2 = vec![vec![3, 0, 0, 0, 2, 2, 2, 2]; 7];
        let f3 = vec![vec![3, 0, 0, 0, 3, 3, 3, 3]; 2];
        let image = crate::subs::pack(&vec![255u8; 20 * 10], 20, 10);
        let subs = vec![crate::subs::SubCue { start: 0.5, end: 1.0, image }];
        let entries: Vec<crate::dvdmenu::MenuEntry> = (0..3)
            .map(|i| crate::dvdmenu::MenuEntry { title: format!("V{i}"), duration_secs: 1.0, thumbnail: crate::frame::Frame { w: 64, h: 40, rgb: vec![9; 64 * 40 * 3] } })
            .collect();
        let pages = crate::dvdmenu::render_pages("Set", &entries);
        let data = build_set("Set", &[video(wide, &f1, &[]), video(tall, &f2, &subs), video(half, &f3, &[])], &pages);
        let p = parse(&data).unwrap();
        assert_eq!(u32::from_le_bytes(data[8..12].try_into().unwrap()), 4);
        assert_eq!(p.video_count, 3);
        assert_eq!(p.title, "Set");
        assert_eq!(p.videos.len(), 3);
        assert_eq!(p.videos[0].layout, wide);
        assert_eq!(p.videos[1].layout, tall);
        assert_eq!(p.videos[2].layout, half);
        assert_eq!(p.videos[1].frames.len(), 7);
        assert_eq!(&p.videos[1].frames[3][..8], &f2[3][..]);
        assert_eq!(p.videos[1].subtitles.len(), 1);
        assert_eq!(p.videos[0].subtitles.len(), 0);
        assert_eq!((p.videos[1].fill_pa, p.videos[1].fill_x, p.videos[1].fill_y), tall.fill());
        assert_eq!(p.videos[2].audio_samples, 1500);
        assert_eq!(p.menu_pages.len(), 1);
        assert_eq!(p.menu_pages[0].0.len(), 512);
        assert_eq!(p.menu_pages[0].1, &pages[0].image[..]);
        assert_eq!(p.layout, wide);
        assert_eq!(p.frames.len(), 5);
    }

    #[test]
    fn single_video_is_v4_with_one_entry() {
        let frames = vec![vec![3, 0, 0, 0, 1, 1, 1, 1]; 4];
        let data = build(&video(layout(), &frames, &[]));
        let p = parse(&data).unwrap();
        assert_eq!(p.video_count, 1);
        assert_eq!(u32::from_le_bytes(data[156..160].try_into().unwrap()), 0);
        assert!(p.menu_pages.is_empty());
        assert_eq!(p.videos[0].frames.len(), 4);
        assert_eq!(p.videos[0].layout, p.layout);
    }
}
