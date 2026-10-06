use crate::kmeans::kmeans;

pub fn to555(r: u8, g: u8, b: u8) -> u16 {
    (r as u16 >> 3) | ((g as u16 >> 3) << 5) | ((b as u16 >> 3) << 10)
}

pub fn expand(c: u16) -> [u8; 3] {
    let f = |v: u16| ((v << 3) | (v >> 2)) as u8;
    [f(c & 31), f((c >> 5) & 31), f((c >> 10) & 31)]
}

pub struct Palette {
    pub colors: [u16; 256],
    lut: Vec<u8>,
}

impl Palette {
    pub fn build(samples: &[[u8; 3]]) -> Palette {
        let data: Vec<[f32; 3]> = samples.iter().map(|c| c.map(|v| v as f32)).collect();
        let mut colors = [0u16; 256];
        for (i, c) in kmeans(&data, 255, 8).iter().enumerate() {
            let c = c.map(|v| v.round().clamp(0.0, 255.0) as u8);
            colors[i + 1] = to555(c[0], c[1], c[2]);
        }
        Palette::from_colors(colors)
    }

    pub fn from_colors(colors: [u16; 256]) -> Palette {
        let rgb: Vec<[i32; 3]> = colors.iter().map(|&c| expand(c).map(|v| v as i32)).collect();
        let lut = (0..32768u16)
            .map(|c| {
                let e = expand(c).map(|v| v as i32);
                let mut best = 0;
                let mut best_d = i32::MAX;
                for (i, p) in rgb.iter().enumerate() {
                    let d = (p[0] - e[0]).pow(2) + (p[1] - e[1]).pow(2) + (p[2] - e[2]).pow(2);
                    if d < best_d {
                        best_d = d;
                        best = i;
                    }
                }
                best as u8
            })
            .collect();
        Palette { colors, lut }
    }

    pub fn index(&self, rgb: [u8; 3]) -> u8 {
        self.lut[to555(rgb[0], rgb[1], rgb[2]) as usize]
    }

    pub fn rgb(&self, i: u8) -> [u8; 3] {
        expand(self.colors[i as usize])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb555_roundtrip() {
        assert_eq!(to555(255, 255, 255), 0x7FFF);
        assert_eq!(to555(255, 0, 0), 0x001F);
        assert_eq!(to555(0, 0, 255), 0x7C00);
        assert_eq!(expand(0x7FFF), [255, 255, 255]);
        assert_eq!(expand(0), [0, 0, 0]);
    }

    #[test]
    fn index_zero_is_black() {
        let p = Palette::build(&[[255, 0, 0]; 10]);
        assert_eq!(p.colors[0], 0);
        assert_eq!(p.rgb(0), [0, 0, 0]);
    }

    #[test]
    fn build_reproduces_distinct_colors() {
        let mut samples = Vec::new();
        for _ in 0..100 {
            samples.extend([[255, 0, 0], [0, 255, 0], [0, 0, 255], [128, 128, 128]]);
        }
        let p = Palette::build(&samples);
        for c in [[255, 0, 0], [0, 255, 0], [0, 0, 255], [128, 128, 128]] {
            assert_eq!(p.rgb(p.index(c)), expand(to555(c[0], c[1], c[2])));
        }
    }

    #[test]
    fn empty_samples_give_black_palette() {
        let p = Palette::build(&[]);
        assert_eq!(p.index([200, 100, 50]), 0);
    }
}
