use crate::palette::to555;
use font8x8::{BASIC_FONTS, UnicodeFonts};

pub const LABELS: [&str; 7] = ["Resume", "Subtitles: On", "Subtitles: Off", "Picture: Original", "Picture: Fill", "Restart", "Main menu"];
pub const TILES_PER_LABEL: usize = 20;
pub const TILE_COUNT: u32 = 140;

pub fn palettes() -> [[u16; 16]; 2] {
    let mut normal = [0u16; 16];
    normal[1] = to555(24, 24, 40);
    normal[2] = to555(255, 255, 255);
    let mut highlight = normal;
    highlight[1] = to555(75, 91, 214);
    [normal, highlight]
}

fn strip(label: &str) -> [[u8; 160]; 8] {
    let mut px = [[1u8; 160]; 8];
    let x0 = (160 - label.len() * 8) / 2;
    for (i, ch) in label.chars().enumerate() {
        if let Some(g) = BASIC_FONTS.get(ch) {
            for (y, bits) in g.iter().enumerate() {
                for c in 0..8 {
                    if (bits >> c) & 1 == 1 {
                        px[y][x0 + i * 8 + c] = 2;
                    }
                }
            }
        }
    }
    px
}

pub fn tiles() -> Vec<[u8; 32]> {
    let mut out = Vec::new();
    for label in LABELS {
        let px = strip(label);
        for t in 0..TILES_PER_LABEL {
            let mut tile = [0u8; 32];
            for y in 0..8 {
                for x in (0..8).step_by(2) {
                    tile[y * 4 + x / 2] = px[y][t * 8 + x] | (px[y][t * 8 + x + 1] << 4);
                }
            }
            out.push(tile);
        }
    }
    out
}

pub fn data() -> Vec<u8> {
    let mut out: Vec<u8> = palettes().iter().flatten().flat_map(|c| c.to_le_bytes()).collect();
    for t in tiles() {
        out.extend_from_slice(&t);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(t: &[u8; 32], x: usize, y: usize) -> u8 {
        let b = t[y * 4 + x / 2];
        if x % 2 == 0 { b & 15 } else { b >> 4 }
    }

    #[test]
    fn data_layout() {
        assert_eq!(data().len(), 64 + 7 * 20 * 32);
        assert_eq!(tiles().len(), TILE_COUNT as usize);
        let [normal, highlight] = palettes();
        assert_ne!(normal[1], highlight[1]);
        assert_eq!(normal[2], 0x7FFF);
    }

    #[test]
    fn labels_are_centred_on_opaque_strips() {
        let t = tiles();
        for label in 0..7 {
            let strip = &t[label * 20..label * 20 + 20];
            let ink: Vec<usize> = (0..160).filter(|&x| (0..8).any(|y| px(&strip[x / 8], x % 8, y) == 2)).collect();
            let (l, r) = (ink[0], 159 - ink[ink.len() - 1]);
            assert!(l.abs_diff(r) <= 8, "label {label}: {l} vs {r}");
            assert!((0..160).all(|x| (0..8).all(|y| px(&strip[x / 8], x % 8, y) != 0)), "label {label} has holes");
        }
    }
}
