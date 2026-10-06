#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub w: usize,
    pub h: usize,
    pub rgb: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub half_res: bool,
    pub w: usize,
    pub h: usize,
    pub x_off: usize,
    pub y_off: usize,
}

impl Layout {
    pub fn fit(src_w: u32, src_h: u32, half_res: bool) -> Layout {
        let (bw, bh) = if half_res { (120usize, 80usize) } else { (240, 160) };
        let (sw, sh) = (src_w.max(1) as usize, src_h.max(1) as usize);
        let (w, h) = if sw * bh >= sh * bw { (bw, bw * sh / sw) } else { (bh * sw / sh, bh) };
        let even = |v: usize| (v / 2 * 2).max(2);
        let (w, h) = (even(w), even(h));
        Layout { half_res, w, h, x_off: ((bw - w) / 2) & !1, y_off: (bh - h) / 2 }
    }

    pub fn fill(&self) -> (u16, i32, i32) {
        let pa = (256.0 * (self.w as f64 / 240.0).min(self.h as f64 / 160.0)).floor() as i32;
        let x = (self.x_off as i32 * 2 + self.w as i32) * 128 - 120 * pa;
        let y = (self.y_off as i32 * 2 + self.h as i32) * 128 - 80 * pa;
        (pa as u16, x, y)
    }

    pub fn original(&self) -> (u16, i32, i32) {
        (if self.half_res { 128 } else { 256 }, 0, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widescreen_is_letterboxed() {
        assert_eq!(Layout::fit(1920, 1080, false), Layout { half_res: false, w: 240, h: 134, x_off: 0, y_off: 13 });
        assert_eq!(Layout::fit(1920, 1080, true), Layout { half_res: true, w: 120, h: 66, x_off: 0, y_off: 7 });
    }

    #[test]
    fn four_by_three_is_pillarboxed() {
        assert_eq!(Layout::fit(640, 480, false), Layout { half_res: false, w: 212, h: 160, x_off: 14, y_off: 0 });
    }

    #[test]
    fn portrait_is_pillarboxed() {
        assert_eq!(Layout::fit(1080, 1920, false), Layout { half_res: false, w: 90, h: 160, x_off: 74, y_off: 0 });
    }

    #[test]
    fn degenerate_sizes_stay_valid() {
        let l = Layout::fit(3, 1000, false);
        assert_eq!((l.w, l.h), (2, 160));
        let l = Layout::fit(0, 0, false);
        assert!(l.w >= 2 && l.h >= 2);
    }

    #[test]
    fn offsets_are_even() {
        for (w, h) in [(1000, 999), (333, 777), (1280, 720), (720, 1280), (500, 500)] {
            for half in [false, true] {
                let l = Layout::fit(w, h, half);
                assert_eq!(l.x_off % 2, 0);
                assert_eq!(l.w % 2, 0);
                assert_eq!(l.h % 2, 0);
                let (bw, bh) = if half { (120, 80) } else { (240, 160) };
                assert!(l.x_off + l.w <= bw && l.y_off + l.h <= bh);
            }
        }
    }

    #[test]
    fn fill_zooms_letterboxed_video() {
        assert_eq!(Layout::fit(1920, 1080, false).fill(), (214, 5040, 3360));
        assert_eq!(Layout::fit(1920, 1080, true).fill(), (105, 2760, 1840));
        assert_eq!(Layout::fit(1080, 1920, false).fill().0, 96);
    }

    #[test]
    fn fill_is_identity_when_already_full() {
        assert_eq!(Layout::fit(240, 160, false).fill(), (256, 0, 0));
        assert_eq!(Layout::fit(240, 160, false).original(), (256, 0, 0));
        assert_eq!(Layout::fit(240, 160, true).original(), (128, 0, 0));
    }

    #[test]
    fn fill_samples_stay_inside_the_video() {
        for (w, h) in [(1920, 1080), (1998, 1080), (2048, 858), (640, 480), (1080, 1920), (240, 160), (1280, 720), (720, 576)] {
            for half in [false, true] {
                let l = Layout::fit(w, h, half);
                let (pa, x, y) = l.fill();
                let pa = pa as i32;
                let (left, right) = (x >> 8, (x + pa * 239) >> 8);
                let (top, bottom) = (y >> 8, (y + pa * 159) >> 8);
                let (x0, y0) = (l.x_off as i32, l.y_off as i32);
                let (x1, y1) = (x0 + l.w as i32 - 1, y0 + l.h as i32 - 1);
                assert!(left >= x0 && right <= x1, "{w}x{h} half={half}: columns {left}..{right} outside {x0}..{x1}");
                assert!(top >= y0 && bottom <= y1, "{w}x{h} half={half}: rows {top}..{bottom} outside {y0}..{y1}");
            }
        }
    }
}
