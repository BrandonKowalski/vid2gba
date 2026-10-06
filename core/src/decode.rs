use crate::frame::Layout;
use crate::palette::expand;

pub struct Decoder {
    layout: Layout,
    pages: [Vec<u8>; 2],
    palette: [u16; 256],
    frames: usize,
}

impl Decoder {
    pub fn new(layout: Layout) -> Decoder {
        Decoder { layout, pages: [vec![0; 240 * 160], vec![0; 240 * 160]], palette: [0; 256], frames: 0 }
    }

    pub fn decode(&mut self, rec: &[u8]) {
        let l = self.layout;
        let page = &mut self.pages[self.frames % 2];
        let cb_len = u16::from_le_bytes([rec[2], rec[3]]) as usize;
        let mut p = 4;
        if rec[0] & 2 != 0 {
            for i in 0..256 {
                self.palette[i] = u16::from_le_bytes([rec[p + 2 * i], rec[p + 2 * i + 1]]);
            }
            p += 512;
        }
        let cb = &rec[p..p + cb_len * 4];
        p += cb_len * 4;
        let bw = l.w / 2;
        let total = bw * (l.h / 2);
        let mut b = 0;
        while b < total {
            let op = rec[p];
            p += 1;
            if op < 0x80 {
                b += op as usize + 1;
            } else {
                for _ in 0..(op - 0x7F) {
                    let e = &cb[rec[p] as usize * 4..][..4];
                    p += 1;
                    let x = l.x_off + (b % bw) * 2;
                    let y = l.y_off + (b / bw) * 2;
                    page[y * 240 + x] = e[0];
                    page[y * 240 + x + 1] = e[1];
                    page[(y + 1) * 240 + x] = e[2];
                    page[(y + 1) * 240 + x + 1] = e[3];
                    b += 1;
                }
            }
        }
        self.frames += 1;
    }

    fn displayed(&self) -> &[u8] {
        &self.pages[(self.frames + 1) % 2]
    }

    pub fn active_rgb(&self) -> Vec<u8> {
        let l = self.layout;
        let page = self.displayed();
        let mut out = Vec::with_capacity(l.w * l.h * 3);
        for y in 0..l.h {
            for x in 0..l.w {
                out.extend(expand(self.palette[page[(l.y_off + y) * 240 + l.x_off + x] as usize]));
            }
        }
        out
    }

    pub fn screen_rgb_affine(&self, pa: i32, x: i32, y: i32) -> Vec<u8> {
        let page = self.displayed();
        let mut out = Vec::with_capacity(240 * 160 * 3);
        for sy in 0..160i32 {
            for sx in 0..240i32 {
                let tx = (x + pa * sx) >> 8;
                let ty = (y + pa * sy) >> 8;
                let i = if (0..240).contains(&tx) && (0..160).contains(&ty) { page[(ty * 240 + tx) as usize] } else { 0 };
                out.extend(expand(self.palette[i as usize]));
            }
        }
        out
    }

    pub fn screen_rgb(&self) -> Vec<u8> {
        let s = if self.layout.half_res { 2 } else { 1 };
        let page = self.displayed();
        let mut out = Vec::with_capacity(240 * 160 * 3);
        for y in 0..160 {
            for x in 0..240 {
                out.extend(expand(self.palette[page[(y / s) * 240 + x / s] as usize]));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_hand_built_records() {
        let layout = Layout { half_res: false, w: 4, h: 2, x_off: 2, y_off: 1 };
        let mut rec = vec![3, 0, 1, 0];
        let mut pal = [0u16; 256];
        pal[1] = 0x7FFF;
        pal[2] = 0x001F;
        for c in pal {
            rec.extend(c.to_le_bytes());
        }
        rec.extend([1, 2, 1, 2]);
        rec.extend([0x81, 0, 0, 0]);
        let mut d = Decoder::new(layout);
        d.decode(&rec);
        let rgb = d.active_rgb();
        assert_eq!(&rgb[0..6], &[255, 255, 255, 255, 0, 0]);
        assert_eq!(&rgb[12..18], &[255, 255, 255, 255, 0, 0]);
        let screen = d.screen_rgb();
        let at = |x: usize, y: usize| &screen[(y * 240 + x) * 3..][..3];
        assert_eq!(at(2, 1), &[255, 255, 255]);
        assert_eq!(at(0, 0), &[0, 0, 0]);
        d.decode(&[0, 0, 0, 0, 0x01, 0, 0, 0]);
        assert_eq!(d.active_rgb(), vec![0; 24]);
        d.decode(&[0, 0, 0, 0, 0x01, 0, 0, 0]);
        assert_eq!(&d.active_rgb()[0..6], &[255, 255, 255, 255, 0, 0]);
    }

    #[test]
    fn affine_identity_matches_screen() {
        let layout = Layout { half_res: false, w: 4, h: 2, x_off: 2, y_off: 1 };
        let mut rec = vec![3, 0, 1, 0];
        let mut pal = [0u16; 256];
        pal[1] = 0x7FFF;
        for c in pal {
            rec.extend(c.to_le_bytes());
        }
        rec.extend([1, 1, 1, 1]);
        rec.extend([0x81, 0, 0, 0]);
        let mut d = Decoder::new(layout);
        d.decode(&rec);
        assert_eq!(d.screen_rgb_affine(256, 0, 0), d.screen_rgb());
        let zoomed = d.screen_rgb_affine(128, 0, 0);
        assert_eq!(&zoomed[(2 * 240 + 4) * 3..][..3], &[255, 255, 255]);
        let shifted = d.screen_rgb_affine(256, -256 * 300, 0);
        assert!(shifted.iter().all(|&v| v == 0));
    }
}
