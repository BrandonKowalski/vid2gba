use crate::container::ascii_title;
use crate::frame::Frame;
use crate::kmeans::{kmeans, nearest};
use crate::palette::{expand, to555};
use crate::title::wrap;
use font8x8::{BASIC_FONTS, UnicodeFonts};

pub const PER_PAGE: usize = 3;
pub const HEADER_H: usize = 24;
pub const ROW_H: usize = 44;
pub const THUMB_W: usize = 64;
pub const THUMB_H: usize = 40;
pub const ROW_BG: u8 = 5;
pub const HIGHLIGHT: u8 = 8;

const HEADER: u8 = 1;
const TEXT: u8 = 2;
const DIM: u8 = 3;
const ACCENT: u8 = 4;

pub struct MenuEntry {
    pub title: String,
    pub duration_secs: f64,
    pub thumbnail: Frame,
}

pub struct MenuPage {
    pub image: Vec<u8>,
    pub palette: [u16; 256],
}

pub fn page_count(n: usize) -> usize {
    n.div_ceil(PER_PAGE)
}

fn fill(img: &mut [u8], x0: usize, y0: usize, x1: usize, y1: usize, c: u8) {
    for y in y0..y1.min(160) {
        for x in x0..x1.min(240) {
            img[y * 240 + x] = c;
        }
    }
}

fn text(img: &mut [u8], s: &str, x0: usize, y: usize, c: u8) {
    for (i, ch) in s.chars().enumerate() {
        if let Some(g) = BASIC_FONTS.get(ch) {
            for (row, bits) in g.iter().enumerate() {
                for col in 0..8 {
                    let x = x0 + i * 8 + col;
                    if (bits >> col) & 1 == 1 && x < 240 && y + row < 160 {
                        img[(y + row) * 240 + x] = c;
                    }
                }
            }
        }
    }
}

fn duration(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    format!("{:02}:{:02}", (s / 60).min(99), s % 60)
}

pub fn render_pages(cart_title: &str, entries: &[MenuEntry]) -> Vec<MenuPage> {
    let pages = page_count(entries.len());
    (0..pages)
        .map(|p| {
            let page = &entries[p * PER_PAGE..((p + 1) * PER_PAGE).min(entries.len())];
            let mut colors = [0u16; 256];
            colors[0] = to555(10, 12, 30);
            colors[HEADER as usize] = to555(36, 44, 96);
            colors[TEXT as usize] = to555(255, 255, 255);
            colors[DIM as usize] = to555(170, 180, 210);
            colors[ACCENT as usize] = to555(255, 200, 0);
            for r in 0..PER_PAGE {
                colors[ROW_BG as usize + r] = to555(22, 28, 60);
            }
            colors[HIGHLIGHT as usize] = to555(75, 91, 214);
            let pixels: Vec<[f32; 3]> = page
                .iter()
                .flat_map(|e| e.thumbnail.rgb.chunks_exact(3).map(|c| [c[0] as f32, c[1] as f32, c[2] as f32]))
                .collect();
            let centroids = kmeans(&pixels, 240, 6);
            for (i, c) in centroids.iter().enumerate() {
                let c = c.map(|v| v.round().clamp(0.0, 255.0) as u8);
                colors[16 + i] = to555(c[0], c[1], c[2]);
            }
            let thumb_pal: Vec<[f32; 3]> = (0..centroids.len()).map(|i| expand(colors[16 + i]).map(|v| v as f32)).collect();
            let mut img = vec![0u8; 240 * 160];
            fill(&mut img, 0, 0, 240, HEADER_H - 4, HEADER);
            let title: String = ascii_title(cart_title).chars().take(24).collect();
            text(&mut img, &title, 8, 6, TEXT);
            if pages > 1 {
                let s = format!("{}/{}", p + 1, pages);
                text(&mut img, &s, 232 - s.len() * 8, 6, ACCENT);
            }
            for (r, e) in page.iter().enumerate() {
                let y = HEADER_H + r * ROW_H;
                fill(&mut img, 2, y, 238, y + ROW_H - 2, ROW_BG + r as u8);
                if !thumb_pal.is_empty() && e.thumbnail.w == THUMB_W && e.thumbnail.h == THUMB_H {
                    for ty in 0..THUMB_H {
                        for tx in 0..THUMB_W {
                            let i = (ty * THUMB_W + tx) * 3;
                            let c = [e.thumbnail.rgb[i] as f32, e.thumbnail.rgb[i + 1] as f32, e.thumbnail.rgb[i + 2] as f32];
                            img[(y + 1 + ty) * 240 + 8 + tx] = 16 + nearest(&thumb_pal, &c) as u8;
                        }
                    }
                }
                for (li, line) in wrap(&ascii_title(&e.title), 19, 2).iter().enumerate() {
                    text(&mut img, line, 80, y + 6 + li * 10, TEXT);
                }
                text(&mut img, &duration(e.duration_secs), 80, y + 28, DIM);
            }
            MenuPage { image: img, palette: colors }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn thumb(c: [u8; 3]) -> Frame {
        Frame { w: THUMB_W, h: THUMB_H, rgb: c.repeat(THUMB_W * THUMB_H) }
    }

    fn entries(n: usize) -> Vec<MenuEntry> {
        (0..n).map(|i| MenuEntry { title: format!("Video number {i}"), duration_secs: 61.0 + i as f64, thumbnail: thumb([200, 30 * i as u8, 40]) }).collect()
    }

    #[test]
    fn pages_hold_three_entries() {
        assert_eq!(page_count(1), 1);
        assert_eq!(page_count(3), 1);
        assert_eq!(page_count(4), 2);
        assert_eq!(render_pages("T", &entries(4)).len(), 2);
    }

    #[test]
    fn rows_use_their_own_background_index() {
        let pages = render_pages("My videos", &entries(4));
        let at = |p: &MenuPage, x: usize, y: usize| p.image[y * 240 + x];
        for r in 0..3 {
            assert_eq!(at(&pages[0], 236, HEADER_H + r * ROW_H + 20), ROW_BG + r as u8);
        }
        assert_eq!(at(&pages[1], 236, HEADER_H + 20), ROW_BG);
        assert_eq!(at(&pages[1], 236, HEADER_H + ROW_H + 20), 0);
        assert_ne!(pages[0].palette[HIGHLIGHT as usize], pages[0].palette[ROW_BG as usize]);
    }

    #[test]
    fn draws_header_titles_durations_and_thumbnails() {
        let page = &render_pages("My videos", &entries(2))[0];
        let header: Vec<u8> = page.image[..20 * 240].to_vec();
        assert!(header.contains(&2));
        let row = &page.image[HEADER_H * 240..(HEADER_H + ROW_H) * 240];
        assert!(row.contains(&2) && row.contains(&3));
        let t = page.image[(HEADER_H + 20) * 240 + 40];
        assert!(t >= 16);
        let rgb = crate::palette::expand(page.palette[t as usize]);
        assert!(rgb[0] > 150 && rgb[2] < 90, "{rgb:?}");
    }

    #[test]
    fn page_indicator_only_with_several_pages() {
        let one = &render_pages("T", &entries(3))[0];
        let two = &render_pages("T", &entries(4))[0];
        let accent = |p: &MenuPage| p.image[..20 * 240].contains(&4);
        assert!(!accent(one));
        assert!(accent(two));
    }
}
