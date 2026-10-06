pub const BLOCK_SAMPLES: usize = 2048;
pub const BLOCK_BYTES: usize = 4 + BLOCK_SAMPLES / 2;

const STEPS: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97,
    107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796,
    876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871,
    5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623,
    27086, 29794, 32767,
];
const INDEX: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];

struct State {
    pred: i32,
    index: i32,
}

impl State {
    fn decode(&mut self, nib: u8) -> i16 {
        let step = STEPS[self.index as usize];
        let mut diff = step >> 3;
        if nib & 4 != 0 {
            diff += step;
        }
        if nib & 2 != 0 {
            diff += step >> 1;
        }
        if nib & 1 != 0 {
            diff += step >> 2;
        }
        if nib & 8 != 0 {
            self.pred -= diff;
        } else {
            self.pred += diff;
        }
        self.pred = self.pred.clamp(-32768, 32767);
        self.index = (self.index + INDEX[nib as usize]).clamp(0, 88);
        self.pred as i16
    }

    fn encode(&mut self, sample: i16) -> u8 {
        let step = STEPS[self.index as usize];
        let mut diff = sample as i32 - self.pred;
        let mut nib = 0u8;
        if diff < 0 {
            nib = 8;
            diff = -diff;
        }
        if diff >= step {
            nib |= 4;
            diff -= step;
        }
        if diff >= step >> 1 {
            nib |= 2;
            diff -= step >> 1;
        }
        if diff >= step >> 2 {
            nib |= 1;
        }
        self.decode(nib);
        nib
    }
}

pub fn encode(samples: &[i16]) -> Vec<u8> {
    let mut st = State { pred: 0, index: 0 };
    let mut out = Vec::with_capacity(samples.len().div_ceil(BLOCK_SAMPLES) * BLOCK_BYTES);
    for chunk in samples.chunks(BLOCK_SAMPLES) {
        out.extend((st.pred as i16).to_le_bytes());
        out.push(st.index as u8);
        out.push(0);
        let mut data = vec![0u8; BLOCK_SAMPLES / 2];
        for (i, &s) in chunk.iter().enumerate() {
            let nib = st.encode(s);
            data[i / 2] |= if i % 2 == 0 { nib } else { nib << 4 };
        }
        out.extend(data);
    }
    out
}

pub fn decode(data: &[u8], n: usize) -> Vec<i16> {
    let mut out = Vec::with_capacity(n);
    let mut st = State { pred: 0, index: 0 };
    for block in data.chunks(BLOCK_BYTES) {
        st.pred = i16::from_le_bytes([block[0], block[1]]) as i32;
        st.index = block[2] as i32;
        for i in 0..BLOCK_SAMPLES {
            if out.len() == n {
                return out;
            }
            let b = block[4 + i / 2];
            out.push(st.decode(if i % 2 == 0 { b & 15 } else { b >> 4 }));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(n: usize) -> Vec<i16> {
        (0..n).map(|i| ((i as f64 * 440.0 * std::f64::consts::TAU / 13379.0).sin() * 10000.0) as i16).collect()
    }

    #[test]
    fn sizes_are_whole_blocks() {
        assert_eq!(encode(&sine(5000)).len(), 3 * BLOCK_BYTES);
        assert_eq!(encode(&[]).len(), 0);
    }

    #[test]
    fn roundtrip_snr() {
        let s = sine(13379);
        let d = decode(&encode(&s), s.len());
        assert_eq!(d.len(), s.len());
        let sig: f64 = s.iter().map(|&v| (v as f64).powi(2)).sum();
        let noise: f64 = s.iter().zip(&d).map(|(&a, &b)| (a as f64 - b as f64).powi(2)).sum();
        let snr = 10.0 * (sig / noise).log10();
        assert!(snr > 20.0, "snr {snr}");
    }

    #[test]
    fn silence_stays_silent() {
        assert!(decode(&encode(&vec![0; 3000]), 3000).iter().all(|&v| v == 0));
    }

    #[test]
    fn extremes_do_not_overflow() {
        let s: Vec<i16> = (0..4096).map(|i| if i % 2 == 0 { i16::MAX } else { i16::MIN }).collect();
        assert_eq!(decode(&encode(&s), s.len()).len(), 4096);
    }
}
