use anyhow::{Context, Result};
use fontdue::{Font, FontSettings};
use vid2gba_core::subs::{Cue, CueImage, MAX_TEXT_W, fit_text, pack};

const BUNDLED: &[u8] = include_bytes!("../assets/NotoSans-Regular.ttf");

pub struct Renderer {
    font: Font,
    missing: usize,
}

impl Renderer {
    pub fn bundled() -> Renderer {
        Renderer { font: Font::from_bytes(BUNDLED, FontSettings::default()).expect("bundled font"), missing: 0 }
    }

    pub fn from_file(path: &std::path::Path) -> Result<Renderer> {
        let bytes = std::fs::read(path).with_context(|| format!("cannot read font {}", path.display()))?;
        let font = Font::from_bytes(bytes, FontSettings::default()).map_err(|e| anyhow::anyhow!("bad font {}: {e}", path.display()))?;
        Ok(Renderer { font, missing: 0 })
    }

    pub fn missing(&self) -> usize {
        self.missing
    }

    fn measure(font: &Font, s: &str, size: f32) -> f32 {
        s.chars().map(|c| font.metrics(c, size).advance_width).sum()
    }

    pub fn render(&mut self, cue: &Cue) -> CueImage {
        let font = &self.font;
        let fitted = fit_text(&cue.lines, |s, size| Self::measure(font, s, size));
        let size = fitted.size;
        let ascent = font.horizontal_line_metrics(size).map_or(size, |m| m.ascent).ceil() as i32;
        let line_h = (size * 1.2).ceil() as i32;
        let widths: Vec<i32> = fitted.lines.iter().map(|l| Self::measure(font, l, size).ceil() as i32).collect();
        let w = (widths.iter().copied().max().unwrap_or(0) + 2).min(MAX_TEXT_W as i32 + 2).max(2) as usize;
        let h = ((line_h * fitted.lines.len() as i32) + 2).min(32) as usize;
        let mut alpha = vec![0u8; w * h];
        let mut missing = 0;
        for (li, line) in fitted.lines.iter().enumerate() {
            let mut x = 1.0 + ((w as i32 - 2 - widths[li]) / 2).max(0) as f32;
            let baseline = 1 + ascent + li as i32 * line_h;
            for ch in line.chars() {
                if font.lookup_glyph_index(ch) == 0 && !ch.is_whitespace() {
                    missing += 1;
                }
                let (m, bmp) = font.rasterize(ch, size);
                let gx = x.round() as i32 + m.xmin;
                let gy = baseline - m.height as i32 - m.ymin;
                for yy in 0..m.height as i32 {
                    for xx in 0..m.width as i32 {
                        let (px, py) = (gx + xx, gy + yy);
                        if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                            let a = &mut alpha[py as usize * w + px as usize];
                            *a = (*a).max(bmp[(yy * m.width as i32 + xx) as usize]);
                        }
                    }
                }
                x += m.advance_width;
            }
        }
        self.missing += missing;
        pack(&alpha, w, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(text: &str) -> Cue {
        Cue { start: 0.0, end: 1.0, lines: vec![text.to_string()] }
    }

    #[test]
    fn renders_latin_within_bounds() {
        let mut r = Renderer::bundled();
        let img = r.render(&cue("Hello, world!"));
        assert!(img.w > 40 && img.w as f32 <= MAX_TEXT_W + 2.0 && img.h <= 32, "{}x{}", img.w, img.h);
        assert!(!img.sprites.is_empty());
        assert_eq!(r.missing(), 0);
    }

    #[test]
    fn long_text_uses_two_lines() {
        let mut r = Renderer::bundled();
        let one = r.render(&cue("Short"));
        let two = r.render(&cue(&"words that keep going ".repeat(4)));
        assert!(two.h > one.h);
        assert!(two.h <= 32);
    }

    #[test]
    fn counts_missing_characters() {
        let mut r = Renderer::bundled();
        r.render(&cue("こんにちは"));
        assert_eq!(r.missing(), 5);
    }
}
