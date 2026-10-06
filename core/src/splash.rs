use crate::adpcm;
use crate::codec::{Params, Streamed, encode_stream};
use crate::fit::SKIP_LOW;
use crate::frame::{Frame, Layout};
use font8x8::{BASIC_FONTS, UnicodeFonts};
use std::sync::OnceLock;

pub const FPS: u32 = 20;
pub const FRAMES: usize = 60;
pub const SPF_Q16: u32 = 43840307;
pub const LAYOUT: Layout = Layout { half_res: false, w: 240, h: 160, x_off: 0, y_off: 0 };

const BG: [u8; 3] = [8, 10, 28];
const TEXT: [u8; 3] = [230, 235, 255];
const FADE_FRAMES: usize = 20;
const RATE: f32 = 13379.0;

pub struct Splash {
    pub records: Vec<Vec<u8>>,
    pub audio: Vec<u8>,
    pub samples: u32,
}

fn fill(f: &mut Frame, x0: usize, y0: usize, x1: usize, y1: usize, c: [u8; 3]) {
    for y in y0..y1.min(f.h) {
        for x in x0..x1.min(f.w) {
            f.rgb[(y * f.w + x) * 3..][..3].copy_from_slice(&c);
        }
    }
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    std::array::from_fn(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t.clamp(0.0, 1.0)).round() as u8)
}

fn ink_columns(g: &[u8; 8]) -> Option<(usize, usize)> {
    let cols: Vec<usize> = (0..8).filter(|&c| g.iter().any(|bits| (bits >> c) & 1 == 1)).collect();
    Some((*cols.first()?, *cols.last()?))
}

fn text_width(s: &str, scale: usize) -> usize {
    let advances: Vec<usize> = s
        .chars()
        .map(|ch| BASIC_FONTS.get(ch).and_then(|g| ink_columns(&g)).map_or(3, |(a, b)| b - a + 1))
        .collect();
    (advances.iter().sum::<usize>() + advances.len().saturating_sub(1)) * scale
}

fn text(f: &mut Frame, s: &str, y0: usize, scale: usize, c: [u8; 3]) {
    let mut x = (f.w - text_width(s, scale).min(f.w)) / 2;
    for ch in s.chars() {
        let Some(g) = BASIC_FONTS.get(ch) else { continue };
        let Some((first, last)) = ink_columns(&g) else {
            x += 4 * scale;
            continue;
        };
        for (row, bits) in g.iter().enumerate() {
            for col in first..=last {
                if (bits >> col) & 1 == 1 {
                    let px = x + (col - first) * scale;
                    let py = y0 + row * scale;
                    fill(f, px, py, px + scale, py + scale, c);
                }
            }
        }
        x += (last - first + 2) * scale;
    }
}

pub fn render(i: usize) -> Frame {
    let mut f = Frame { w: 240, h: 160, rgb: BG.repeat(240 * 160) };
    let t = i.min(FADE_FRAMES) as f32 / FADE_FRAMES as f32;
    if t > 0.0 {
        let c = if t >= 1.0 { TEXT } else { mix(BG, TEXT, t) };
        text(&mut f, "vid2gba", 68, 3, c);
    }
    f
}

pub fn chime() -> Vec<i16> {
    let notes = [(1.0f32, 523.25f32), (1.15, 659.25), (1.3, 783.99), (1.45, 1046.5)];
    (0..3 * 13379)
        .map(|n| {
            let t = n as f32 / RATE;
            let v: f32 = notes
                .iter()
                .filter(|(start, _)| t >= *start)
                .map(|(start, freq)| {
                    let dt = t - start;
                    let env = (-3.0 * dt).exp();
                    let w = std::f32::consts::TAU * freq * dt;
                    env * (w.sin() + 0.3 * (2.0 * w).sin())
                })
                .sum();
            (v * 0.18 * 32767.0).round().clamp(-32768.0, 32767.0) as i16
        })
        .collect()
}

pub fn encoded() -> &'static Splash {
    static SPLASH: OnceLock<Splash> = OnceLock::new();
    SPLASH.get_or_init(|| {
        let frames = (0..FRAMES).map(|i| Ok(render(i)));
        let params = Params { codebook: 256, skip: SKIP_LOW };
        let records = match encode_stream(frames, FPS, &params, usize::MAX).expect("splash encodes") {
            Streamed::Fit(r) => r,
            Streamed::Over { .. } => unreachable!(),
        };
        let pcm = chime();
        Splash { records, audio: adpcm::encode(&pcm), samples: pcm.len() as u32 }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::Decoder;

    #[test]
    fn spf_constant_matches_formula() {
        assert_eq!(SPF_Q16 as u64, (13379u64 << 16) / FPS as u64);
    }

    #[test]
    fn splash_is_text_on_plain_background() {
        let first = render(0);
        assert_eq!((first.w, first.h, first.rgb.len()), (240, 160, 240 * 160 * 3));
        assert!(first.rgb.chunks(3).all(|p| p == BG), "first frame is not plain background");
        let last = render(FRAMES - 1);
        let text: Vec<&[u8]> = last.rgb.chunks(3).filter(|p| *p != BG).collect();
        assert!(text.len() > 500, "{}", text.len());
        assert!(text.iter().all(|p| *p == TEXT), "only background and text colours allowed");
        assert_ne!(render(5).rgb, render(10).rgb);
        assert_eq!(render(30).rgb, render(FRAMES - 1).rgb);
    }

    #[test]
    fn chime_is_three_seconds_and_audible() {
        let c = chime();
        assert_eq!(c.len(), 3 * 13379);
        assert!(c.iter().any(|&s| s.abs() > 3000));
        assert!(c[..13379 / 2].iter().all(|&s| s == 0));
    }

    #[test]
    fn splash_is_deterministic_and_small() {
        let s = encoded();
        assert_eq!(s.records.len(), FRAMES);
        assert_eq!(s.samples, 3 * 13379);
        let size: usize = s.records.iter().map(|r| r.len() + 4).sum::<usize>() + s.audio.len();
        assert!(size < 100_000, "{size}");
        let again: Vec<Frame> = (0..FRAMES).map(render).collect();
        assert_eq!(again[30], render(30));
    }

    #[test]
    fn encoded_splash_decodes_close_to_source() {
        let s = encoded();
        let mut d = Decoder::new(LAYOUT);
        for r in &s.records {
            d.decode(r);
        }
        let got = d.active_rgb();
        let want = render(FRAMES - 1).rgb;
        let mse: f64 = got.iter().zip(&want).map(|(&a, &b)| (a as f64 - b as f64).powi(2)).sum::<f64>() / got.len() as f64;
        assert!(mse < 200.0, "{mse}");
    }

    #[test]
    fn intro_letters_are_evenly_spaced_and_centred() {
        let f = render(FRAMES - 1);
        let ink: Vec<bool> = (0..f.w).map(|x| (0..f.h).any(|y| &f.rgb[(y * f.w + x) * 3..][..3] == TEXT)).collect();
        let mut runs = Vec::new();
        let mut x = 0;
        while x < f.w {
            if ink[x] {
                let start = x;
                while x < f.w && ink[x] {
                    x += 1;
                }
                runs.push((start, x));
            } else {
                x += 1;
            }
        }
        assert_eq!(runs.len(), 7, "{runs:?}");
        let gaps: Vec<usize> = runs.windows(2).map(|w| w[1].0 - w[0].1).collect();
        assert!(gaps.iter().all(|&g| g == gaps[0]), "uneven gaps {gaps:?}");
        let left = runs[0].0;
        let right = f.w - runs[6].1;
        assert!(left.abs_diff(right) <= 3, "not centred: {left} vs {right}");
    }
}
