use anyhow::{Result, anyhow, ensure};
use crate::palette::to555;

#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub lines: Vec<String>,
}

fn parse_timestamp(s: &str) -> Option<f64> {
    let (hms, frac) = s.trim().rsplit_once([',', '.'])?;
    if frac.len() != 3 || !frac.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let parts: Vec<u64> = hms.split(':').map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let (h, m, sec) = match parts.as_slice() {
        [h, m, s] => (*h, *m, *s),
        [m, s] => (0, *m, *s),
        _ => return None,
    };
    if m >= 60 || sec >= 60 {
        return None;
    }
    Some((h * 3600 + m * 60 + sec) as f64 + frac.parse::<f64>().ok()? / 1000.0)
}

fn clean(line: &str) -> String {
    let mut out = String::new();
    let (mut angle, mut brace) = (false, false);
    for c in line.chars() {
        match c {
            '<' => angle = true,
            '>' if angle => angle = false,
            '{' => brace = true,
            '}' if brace => brace = false,
            _ if angle || brace => {}
            _ => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn parse(text: &str) -> Result<Vec<Cue>> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text).replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.lines().collect();
    let mut cues = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some((a, b)) = lines[i].split_once("-->") else {
            i += 1;
            continue;
        };
        let bad = |t: &str| anyhow!("line {}: bad timestamp '{}'", i + 1, t.trim());
        let start = parse_timestamp(a).ok_or_else(|| bad(a))?;
        let end_token = b.split_whitespace().next().unwrap_or("");
        let end = parse_timestamp(end_token).ok_or_else(|| bad(end_token))?;
        i += 1;
        let mut text_lines = Vec::new();
        while i < lines.len() && !lines[i].trim().is_empty() {
            let c = clean(lines[i]);
            if !c.is_empty() {
                text_lines.push(c);
            }
            i += 1;
        }
        if !text_lines.is_empty() && end > start {
            cues.push(Cue { start, end, lines: text_lines });
        }
    }
    ensure!(!cues.is_empty(), "no subtitle cues found");
    cues.sort_by(|a, b| a.start.total_cmp(&b.start));
    for k in 1..cues.len() {
        let next = cues[k].start;
        if cues[k - 1].end > next {
            cues[k - 1].end = next;
        }
    }
    cues.retain(|c| c.end > c.start);
    Ok(cues)
}

pub fn trim(cues: &[Cue], start: f64, end: f64) -> Vec<Cue> {
    cues.iter()
        .filter(|c| c.end > start && c.start < end)
        .map(|c| Cue { start: c.start.max(start) - start, end: c.end.min(end) - start, lines: c.lines.clone() })
        .collect()
}

pub const MAX_TEXT_W: f32 = 232.0;
pub const SIZES: [f32; 4] = [12.0, 11.0, 10.0, 9.0];

pub struct Fitted {
    pub size: f32,
    pub lines: Vec<String>,
}

fn wrap(lines: &[String], size: f32, measure: &impl Fn(&str, f32) -> f32) -> Vec<String> {
    let mut out = Vec::new();
    for line in lines {
        let mut cur = String::new();
        for word in line.split_whitespace() {
            let candidate = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if measure(&candidate, size) <= MAX_TEXT_W {
                cur = candidate;
                continue;
            }
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            for ch in word.chars() {
                let mut next = cur.clone();
                next.push(ch);
                if !cur.is_empty() && measure(&next, size) > MAX_TEXT_W {
                    out.push(std::mem::take(&mut cur));
                    cur.push(ch);
                } else {
                    cur = next;
                }
            }
        }
        if !cur.is_empty() {
            out.push(cur);
        }
    }
    out
}

pub fn fit_text(lines: &[String], measure: impl Fn(&str, f32) -> f32) -> Fitted {
    for size in SIZES {
        let wrapped = wrap(lines, size, &measure);
        if wrapped.len() <= 2 {
            return Fitted { size, lines: wrapped };
        }
    }
    let size = SIZES[SIZES.len() - 1];
    let mut wrapped = wrap(lines, size, &measure);
    wrapped.truncate(2);
    let last = &mut wrapped[1];
    while !last.is_empty() && measure(&format!("{}…", last.trim_end()), size) > MAX_TEXT_W {
        last.pop();
    }
    *last = format!("{}…", last.trim_end());
    Fitted { size, lines: wrapped }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CueImage {
    pub w: u8,
    pub h: u8,
    pub sprites: Vec<(u8, u8, [u8; 256])>,
}

pub struct SubCue {
    pub start: f64,
    pub end: f64,
    pub image: CueImage,
}

pub fn palette() -> [u16; 16] {
    let mut p = [0u16; 16];
    for (k, c) in p.iter_mut().enumerate().skip(2) {
        let v = ((k - 1) * 255 / 14) as u8;
        *c = to555(v, v, v);
    }
    p
}

pub fn pack(alpha: &[u8], w: usize, h: usize) -> CueImage {
    let (cw, ch) = (w.min(255), h.min(32));
    let at = |x: isize, y: isize| -> u8 {
        if x < 0 || y < 0 || x >= cw as isize || y >= ch as isize { 0 } else { alpha[y as usize * w + x as usize] }
    };
    let index = |x: usize, y: usize| -> u8 {
        let a = at(x as isize, y as isize);
        if a >= 32 {
            return (1 + ((a as u32 * 14 + 127) / 255) as u8).clamp(2, 15);
        }
        let mut o = 0;
        for dy in -1..=1 {
            for dx in -1..=1 {
                o = o.max(at(x as isize + dx, y as isize + dy));
            }
        }
        if o >= 64 { 1 } else { 0 }
    };
    let mut sprites = Vec::new();
    for sy in (0..ch).step_by(16) {
        for sx in (0..cw).step_by(32) {
            let mut tiles = [0u8; 256];
            let mut any = false;
            for ly in 0..16 {
                for lx in 0..32 {
                    let (x, y) = (sx + lx, sy + ly);
                    if x >= cw || y >= ch {
                        continue;
                    }
                    let v = index(x, y);
                    if v == 0 {
                        continue;
                    }
                    any = true;
                    let tile = (ly / 8) * 4 + lx / 8;
                    let b = &mut tiles[tile * 32 + (ly % 8) * 4 + (lx % 8) / 2];
                    *b |= if lx % 2 == 0 { v } else { v << 4 };
                }
            }
            if any {
                sprites.push((sx as u8, sy as u8, tiles));
            }
        }
    }
    CueImage { w: cw as u8, h: ch as u8, sprites }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_srt_and_strips_tags() {
        let c = parse("1\n00:00:01,000 --> 00:00:02,500\nHello\n<i>world</i>\n\n2\n00:00:03,000 --> 00:00:04,000\nBye\n").unwrap();
        assert_eq!(c.len(), 2);
        assert_eq!(c[0], Cue { start: 1.0, end: 2.5, lines: vec!["Hello".into(), "world".into()] });
        assert_eq!(c[1].lines, vec!["Bye".to_string()]);
    }

    #[test]
    fn parses_vtt_with_header_notes_and_settings() {
        let c = parse("WEBVTT\n\nNOTE a comment\n\nintro\n00:01.000 --> 00:02.000 align:start\n{\\an8}Hi   there\n").unwrap();
        assert_eq!(c, vec![Cue { start: 1.0, end: 2.0, lines: vec!["Hi there".into()] }]);
    }

    #[test]
    fn handles_bom_and_crlf() {
        let c = parse("\u{feff}1\r\n00:00:00,500 --> 00:00:01,000\r\nA\r\n").unwrap();
        assert_eq!(c[0].start, 0.5);
        assert_eq!(c[0].lines, vec!["A".to_string()]);
    }

    #[test]
    fn sorts_and_clamps_overlaps() {
        let c = parse("2\n00:00:03,000 --> 00:00:05,000\nB\n\n1\n00:00:01,000 --> 00:00:04,000\nA\n").unwrap();
        assert_eq!((c[0].start, c[0].end), (1.0, 3.0));
        assert_eq!((c[1].start, c[1].end), (3.0, 5.0));
    }

    #[test]
    fn rejects_bad_timestamp_with_line_number() {
        let e = parse("1\n00:00:01 --> 00:00:02,000\nx\n").unwrap_err().to_string();
        assert!(e.contains("line 2"), "{e}");
        assert!(parse("").is_err());
        assert!(parse("WEBVTT\n").is_err());
    }

    #[test]
    fn trim_shifts_drops_and_clips() {
        let cues = vec![
            Cue { start: 0.0, end: 1.0, lines: vec!["a".into()] },
            Cue { start: 1.5, end: 3.0, lines: vec!["b".into()] },
            Cue { start: 9.0, end: 10.0, lines: vec!["c".into()] },
        ];
        let t = trim(&cues, 2.0, 5.0);
        assert_eq!(t, vec![Cue { start: 0.0, end: 1.0, lines: vec!["b".into()] }]);
    }

    fn mono(s: &str, size: f32) -> f32 {
        s.chars().count() as f32 * size * 0.5
    }

    #[test]
    fn short_line_keeps_full_size() {
        let f = fit_text(&["Hello".into()], mono);
        assert_eq!((f.size, f.lines.clone()), (12.0, vec!["Hello".to_string()]));
    }

    #[test]
    fn long_line_wraps_then_shrinks_then_ellipsizes() {
        let words = "word ".repeat(12);
        let f = fit_text(&[words.trim().into()], mono);
        assert_eq!(f.lines.len(), 2);
        assert!(f.lines.iter().all(|l| mono(l, f.size) <= MAX_TEXT_W));
        let huge = "word ".repeat(200);
        let f = fit_text(&[huge.trim().into()], mono);
        assert_eq!(f.size, 9.0);
        assert_eq!(f.lines.len(), 2);
        assert!(f.lines[1].ends_with('…'));
        assert!(mono(&f.lines[1], 9.0) <= MAX_TEXT_W);
    }

    #[test]
    fn wraps_cjk_by_character() {
        let f = fit_text(&["あ".repeat(50)], |s, size| s.chars().count() as f32 * size);
        assert!(f.lines.len() <= 2);
        assert!(f.lines.iter().all(|l| l.chars().count() as f32 * f.size <= MAX_TEXT_W));
    }

    #[test]
    fn palette_ramps_to_white() {
        let p = palette();
        assert_eq!(p[0], 0);
        assert_eq!(p[1], 0);
        assert_eq!(p[15], 0x7FFF);
        assert!(p[8] > 0 && p[8] < 0x7FFF);
    }

    fn px(img: &CueImage, x: usize, y: usize) -> u8 {
        let (sx, sy) = (x / 32 * 32, y / 16 * 16);
        let Some((_, _, t)) = img.sprites.iter().find(|(a, b, _)| *a as usize == sx && *b as usize == sy) else { return 0 };
        let (lx, ly) = (x - sx, y - sy);
        let tile = (ly / 8) * 4 + lx / 8;
        let b = t[tile * 32 + (ly % 8) * 4 + (lx % 8) / 2];
        if lx % 2 == 0 { b & 15 } else { b >> 4 }
    }

    #[test]
    fn pack_outlines_and_skips_empty_sprites() {
        let (w, h) = (64, 16);
        let mut alpha = vec![0u8; w * h];
        for y in 6..10 {
            for x in 6..10 {
                alpha[y * w + x] = 255;
            }
        }
        let img = pack(&alpha, w, h);
        assert_eq!((img.w, img.h), (64, 16));
        assert_eq!(img.sprites.len(), 1);
        assert_eq!(px(&img, 7, 7), 15);
        assert_eq!(px(&img, 5, 7), 1);
        assert_eq!(px(&img, 2, 2), 0);
    }

    #[test]
    fn typical_line_is_small() {
        let (w, h) = (200, 18);
        let alpha: Vec<u8> = (0..w * h).map(|i| if (i / 3) % 2 == 0 && (i / w) % 15 > 2 { 255 } else { 0 }).collect();
        let img = pack(&alpha, w, h);
        assert!(img.sprites.len() * 260 <= 4200, "{}", img.sprites.len());
    }
}
