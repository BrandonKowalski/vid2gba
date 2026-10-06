use crate::palette::to555;
use font8x8::{BASIC_FONTS, UnicodeFonts};

pub fn wrap(text: &str, width: usize, max_lines: usize) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let mut word = word;
        while word.len() > width {
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            lines.push(word[..width].to_string());
            word = &word[width..];
        }
        if cur.is_empty() {
            cur = word.to_string();
        } else if cur.len() + 1 + word.len() <= width {
            cur.push(' ');
            cur.push_str(word);
        } else {
            lines.push(std::mem::take(&mut cur));
            cur = word.to_string();
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let last = &mut lines[max_lines - 1];
        last.truncate(width - 3);
        last.push_str("...");
    }
    lines
}

fn draw(img: &mut [u8], text: &str, y: usize, color: u8) {
    let x0 = (240 - text.len() * 8) / 2;
    for (i, ch) in text.chars().enumerate() {
        if let Some(glyph) = BASIC_FONTS.get(ch) {
            for (row, bits) in glyph.iter().enumerate() {
                for col in 0..8 {
                    if (bits >> col) & 1 == 1 {
                        img[(y + row) * 240 + x0 + i * 8 + col] = color;
                    }
                }
            }
        }
    }
}

pub fn render(title: &str) -> (Vec<u8>, [u16; 256]) {
    let mut colors = [0u16; 256];
    colors[0] = to555(16, 24, 64);
    colors[1] = to555(255, 255, 255);
    colors[2] = to555(255, 200, 0);
    let mut img = vec![0u8; 240 * 160];
    let lines = wrap(title, 28, 8);
    let top = 64 - lines.len() * 10 / 2;
    for (i, line) in lines.iter().enumerate() {
        draw(&mut img, line, top + i * 10, 1);
    }
    draw(&mut img, "PRESS START", 136, 2);
    (img, colors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_words() {
        assert_eq!(wrap("hello world", 5, 8), vec!["hello", "world"]);
        assert_eq!(wrap("a b c", 28, 8), vec!["a b c"]);
        assert!(wrap("", 28, 8).is_empty());
    }

    #[test]
    fn splits_long_words() {
        assert_eq!(wrap("abcdefghij", 4, 8), vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn wrap_truncates_with_ellipsis() {
        let lines = wrap(&"word ".repeat(100), 28, 3);
        assert_eq!(lines.len(), 3);
        assert!(lines[2].ends_with("..."));
        assert!(lines.iter().all(|l| l.len() <= 28));
    }

    #[test]
    fn renders_title_and_prompt() {
        let (img, pal) = render("Hi");
        assert_eq!(img.len(), 240 * 160);
        assert!(img.contains(&1));
        assert!(img.contains(&2));
        assert_ne!(pal[1], pal[0]);
        let (img, _) = render("");
        assert!(!img.contains(&1));
        assert!(img.contains(&2));
    }
}
