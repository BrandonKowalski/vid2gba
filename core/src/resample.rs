const TAPS: usize = 65;

pub struct Resampler {
    step: f64,
    next: f64,
    buf: Vec<f32>,
    base: usize,
    consumed: usize,
    taps: Vec<f32>,
}

impl Resampler {
    pub fn new(in_rate: f64, out_rate: f64) -> Resampler {
        let fc = (0.45 * out_rate / in_rate).min(0.45);
        let half = (TAPS / 2) as f64;
        let mut taps: Vec<f32> = (0..TAPS)
            .map(|i| {
                let x = i as f64 - half;
                let sinc = if x == 0.0 { 2.0 * fc } else { (std::f64::consts::TAU * fc * x).sin() / (std::f64::consts::PI * x) };
                let w = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / (TAPS - 1) as f64).cos();
                (sinc * w) as f32
            })
            .collect();
        let sum: f32 = taps.iter().sum();
        taps.iter_mut().for_each(|t| *t /= sum);
        Resampler { step: in_rate / out_rate, next: 0.0, buf: vec![0.0; TAPS / 2], base: 0, consumed: 0, taps }
    }

    fn filtered(&self, i: usize) -> f32 {
        let s = i - self.base;
        self.taps.iter().zip(&self.buf[s..s + TAPS]).map(|(a, b)| a * b).sum()
    }

    fn drain(&mut self) -> Vec<i16> {
        let mut out = Vec::new();
        loop {
            let i = self.next.floor() as usize;
            if i >= self.consumed || i + 1 - self.base + TAPS > self.buf.len() {
                break;
            }
            let frac = (self.next - i as f64) as f32;
            let v = self.filtered(i) * (1.0 - frac) + self.filtered(i + 1) * frac;
            out.push((v * 32767.0).round().clamp(-32768.0, 32767.0) as i16);
            self.next += self.step;
        }
        let keep_from = (self.next.floor() as usize).saturating_sub(self.base).min(self.buf.len());
        if keep_from > 0 {
            self.buf.drain(..keep_from);
            self.base += keep_from;
        }
        out
    }

    pub fn push(&mut self, input: &[f32]) -> Vec<i16> {
        self.buf.extend_from_slice(input);
        self.consumed += input.len();
        self.drain()
    }

    pub fn finish(&mut self) -> Vec<i16> {
        self.buf.extend(std::iter::repeat_n(0.0, TAPS + 1));
        self.drain()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f64, rate: f64, secs: f64, amp: f32) -> Vec<f32> {
        (0..(rate * secs) as usize).map(|i| (amp as f64 * (i as f64 * freq * std::f64::consts::TAU / rate).sin()) as f32).collect()
    }

    fn rms(v: &[i16]) -> f64 {
        (v.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / v.len() as f64).sqrt()
    }

    fn crossings(v: &[i16]) -> usize {
        v.windows(2).filter(|w| (w[0] < 0) != (w[1] < 0)).count()
    }

    fn run(r: &mut Resampler, input: &[f32]) -> Vec<i16> {
        let mut out = r.push(input);
        out.extend(r.finish());
        out
    }

    #[test]
    fn keeps_frequency_and_length() {
        let out = run(&mut Resampler::new(48000.0, 13379.0), &sine(1000.0, 48000.0, 1.0, 0.5));
        assert!((out.len() as i64 - 13379).abs() <= 2, "{}", out.len());
        let c = crossings(&out[200..]) as f64;
        assert!((c - 2000.0 * (13179.0 / 13379.0)).abs() < 25.0, "{c}");
        assert!((rms(&out[200..]) - 0.5 * 32767.0 / 2f64.sqrt()).abs() < 1500.0);
    }

    #[test]
    fn attenuates_above_nyquist() {
        let out = run(&mut Resampler::new(48000.0, 13379.0), &sine(8000.0, 48000.0, 1.0, 0.5));
        assert!(rms(&out[200..]) < 0.2 * 0.5 * 32767.0 / 2f64.sqrt(), "{}", rms(&out));
    }

    #[test]
    fn chunked_equals_whole() {
        let input = sine(440.0, 44100.0, 0.5, 0.3);
        let whole = run(&mut Resampler::new(44100.0, 13379.0), &input);
        let mut r = Resampler::new(44100.0, 13379.0);
        let mut chunked = Vec::new();
        for c in input.chunks(1000) {
            chunked.extend(r.push(c));
        }
        chunked.extend(r.finish());
        assert_eq!(chunked, whole);
    }

    #[test]
    fn upsamples() {
        let out = run(&mut Resampler::new(8000.0, 13379.0), &sine(440.0, 8000.0, 1.0, 0.3));
        assert!((out.len() as i64 - 13379).abs() <= 2);
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert!(run(&mut Resampler::new(48000.0, 13379.0), &[]).is_empty());
    }
}
