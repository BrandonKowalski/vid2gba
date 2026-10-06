use crate::frame::Frame;

pub const GAMMA: f32 = 0.6;
pub const SATURATION: f32 = 1.2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Emulator,
    Gba,
}

pub fn apply(frame: &mut Frame, target: Target) {
    if target == Target::Emulator {
        return;
    }
    let lut: [f32; 256] = std::array::from_fn(|i| 255.0 * (i as f32 / 255.0).powf(GAMMA));
    for px in frame.rgb.chunks_exact_mut(3) {
        let [r, g, b] = [lut[px[0] as usize], lut[px[1] as usize], lut[px[2] as usize]];
        let y = 0.299 * r + 0.587 * g + 0.114 * b;
        for (c, v) in px.iter_mut().zip([r, g, b]) {
            *c = (y + (v - y) * SATURATION).round().clamp(0.0, 255.0) as u8;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Frame;

    fn toned(c: [u8; 3]) -> [u8; 3] {
        let mut f = Frame { w: 1, h: 1, rgb: c.to_vec() };
        apply(&mut f, Target::Gba);
        [f.rgb[0], f.rgb[1], f.rgb[2]]
    }

    fn sat(c: [f32; 3]) -> f32 {
        let max = c.iter().cloned().fold(0.0, f32::max);
        let min = c.iter().cloned().fold(255.0, f32::min);
        if max == 0.0 { 0.0 } else { (max - min) / max }
    }

    #[test]
    fn emulator_is_identity() {
        let mut f = Frame { w: 1, h: 1, rgb: vec![10, 128, 250] };
        apply(&mut f, Target::Emulator);
        assert_eq!(f.rgb, vec![10, 128, 250]);
    }

    #[test]
    fn gba_keeps_black_and_white() {
        assert_eq!(toned([0, 0, 0]), [0, 0, 0]);
        assert_eq!(toned([255, 255, 255]), [255, 255, 255]);
    }

    #[test]
    fn gba_brightens_mid_gray_and_keeps_it_gray() {
        let t = toned([128, 128, 128]);
        assert!(t[0] >= 160, "{t:?}");
        assert!(t[0] == t[1] && t[1] == t[2]);
    }

    #[test]
    fn gba_is_monotonic_for_grays() {
        let mut prev = 0;
        for v in 0..=255u8 {
            let t = toned([v, v, v])[0];
            assert!(t >= prev, "{v}");
            prev = t;
        }
    }

    #[test]
    fn gba_raises_saturation_over_gamma_alone() {
        let c = [180u8, 90, 60];
        let lifted = c.map(|v| 255.0 * (v as f32 / 255.0).powf(GAMMA));
        let t = toned(c).map(|v| v as f32);
        assert!(sat(t) > sat(lifted), "{t:?} vs {lifted:?}");
    }

    #[test]
    fn gba_is_stable_under_repeat() {
        let mut f = Frame { w: 2, h: 1, rgb: vec![250, 5, 128, 255, 0, 255] };
        apply(&mut f, Target::Gba);
        apply(&mut f, Target::Gba);
        assert_eq!(f.rgb.len(), 6);
        assert_eq!(&f.rgb[3..], &[255, 0, 255]);
    }
}
