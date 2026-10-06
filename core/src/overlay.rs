use crate::palette::to555;
use font8x8::{BASIC_FONTS, UnicodeFonts};

pub const T_COLON: u8 = 10;
pub const T_SLASH: u8 = 11;
pub const T_BLANK: u8 = 12;
pub const T_BAR: u8 = 13;
pub const T_PAUSE: u8 = 22;
pub const TILE_COUNT: u32 = 23;

const TEXT: u8 = 1;
const BACK: u8 = 2;
const FILL: u8 = 3;
const EMPTY: u8 = 4;

type Pixels = [[u8; 8]; 8];

pub fn palette() -> [u16; 16] {
    let mut p = [0u16; 16];
    p[TEXT as usize] = to555(255, 255, 255);
    p[BACK as usize] = to555(16, 16, 16);
    p[FILL as usize] = to555(255, 200, 0);
    p[EMPTY as usize] = to555(90, 90, 90);
    p
}

fn pack(px: &Pixels) -> [u8; 32] {
    let mut out = [0u8; 32];
    for y in 0..8 {
        for x in (0..8).step_by(2) {
            out[y * 4 + x / 2] = px[y][x] | (px[y][x + 1] << 4);
        }
    }
    out
}

fn glyph(ch: char) -> Pixels {
    let mut px = [[BACK; 8]; 8];
    if let Some(g) = BASIC_FONTS.get(ch) {
        for (y, bits) in g.iter().enumerate() {
            for x in 0..8 {
                if (bits >> x) & 1 == 1 {
                    px[y][x] = TEXT;
                }
            }
        }
    }
    px
}

fn bar(k: usize) -> Pixels {
    let mut px = [[BACK; 8]; 8];
    for row in px.iter_mut().take(6).skip(2) {
        for (x, p) in row.iter_mut().enumerate() {
            *p = if x < k { FILL } else { EMPTY };
        }
    }
    px
}

fn pause() -> Pixels {
    let mut px = [[BACK; 8]; 8];
    for row in px.iter_mut().take(7).skip(1) {
        for x in [2, 3, 5, 6] {
            row[x] = TEXT;
        }
    }
    px
}

pub fn tiles() -> Vec<[u8; 32]> {
    let mut t: Vec<[u8; 32]> = "0123456789:/".chars().map(|c| pack(&glyph(c))).collect();
    t.push(pack(&[[BACK; 8]; 8]));
    t.extend((0..=8).map(|k| pack(&bar(k))));
    t.push(pack(&pause()));
    t
}

pub fn data() -> Vec<u8> {
    let mut out: Vec<u8> = palette().iter().flat_map(|c| c.to_le_bytes()).collect();
    for t in tiles() {
        out.extend_from_slice(&t);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(tile: &[u8; 32], x: usize, y: usize) -> u8 {
        let b = tile[y * 4 + x / 2];
        if x % 2 == 0 { b & 15 } else { b >> 4 }
    }

    #[test]
    fn data_layout() {
        let d = data();
        assert_eq!(d.len(), 32 + TILE_COUNT as usize * 32);
        assert_eq!(u16::from_le_bytes([d[0], d[1]]), 0);
        assert_eq!(tiles().len(), TILE_COUNT as usize);
    }

    #[test]
    fn text_tiles_have_dark_backing() {
        let t = tiles();
        for i in 0..=T_SLASH as usize {
            let px: Vec<u8> = (0..64).map(|p| pixel(&t[i], p % 8, p / 8)).collect();
            assert!(px.iter().all(|&c| c == TEXT || c == BACK), "tile {i}");
            assert!(px.contains(&TEXT), "tile {i} has no glyph");
        }
        assert!((0..64).all(|p| pixel(&t[T_BLANK as usize], p % 8, p / 8) == BACK));
    }

    #[test]
    fn bar_tiles_fill_left_to_right() {
        let t = tiles();
        for k in 0..=8usize {
            let tile = &t[T_BAR as usize + k];
            for x in 0..8 {
                assert_eq!(pixel(tile, x, 4), if x < k { FILL } else { EMPTY }, "k={k} x={x}");
            }
            assert_eq!(pixel(tile, 0, 0), BACK);
        }
    }

    #[test]
    fn pause_icon_has_two_bars() {
        let t = &tiles()[T_PAUSE as usize];
        assert_eq!(pixel(t, 2, 4), TEXT);
        assert_eq!(pixel(t, 5, 4), TEXT);
        assert_eq!(pixel(t, 4, 4), BACK);
    }
}
