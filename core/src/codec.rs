use crate::frame::Frame;
use crate::kmeans::{dist, kmeans, nearest};
use crate::palette::Palette;
use rayon::prelude::*;
use anyhow::Result;

type Vec12 = [f32; 12];

pub struct Params {
    pub codebook: usize,
    pub skip: u32,
}

pub enum Streamed {
    Fit(Vec<Vec<u8>>),
    Over { frames_fit: usize },
}

fn scene_cut(a: &Frame, b: &Frame) -> bool {
    let pixels = a.rgb.len() / 3;
    let step = (pixels / 4096).max(1);
    let mut total = 0u64;
    let mut count = 0u64;
    for p in (0..pixels).step_by(step) {
        for c in 0..3 {
            total += (a.rgb[p * 3 + c] as i32 - b.rgb[p * 3 + c] as i32).unsigned_abs() as u64;
        }
        count += 3;
    }
    total / count.max(1) > 40
}

pub const GOP_SECONDS: u32 = 2;

pub struct GopSplitter {
    max_len: usize,
    gop: Vec<Frame>,
}

impl GopSplitter {
    pub fn new(fps: u32) -> GopSplitter {
        GopSplitter { max_len: (fps * GOP_SECONDS).max(1) as usize, gop: Vec::new() }
    }

    pub fn push(&mut self, frame: Frame) -> Option<Vec<Frame>> {
        let done = self.gop.last().is_some_and(|last| self.gop.len() >= self.max_len || scene_cut(last, &frame));
        let out = if done { Some(std::mem::take(&mut self.gop)) } else { None };
        self.gop.push(frame);
        out
    }

    pub fn finish(&mut self) -> Option<Vec<Frame>> {
        if self.gop.is_empty() { None } else { Some(std::mem::take(&mut self.gop)) }
    }
}

pub struct Budget {
    limit: usize,
    used: usize,
    records: Vec<Vec<u8>>,
    over: bool,
}

impl Budget {
    pub fn new(limit: usize) -> Budget {
        Budget { limit, used: 0, records: Vec::new(), over: false }
    }

    pub fn accept(&mut self, records: Vec<Vec<u8>>) -> bool {
        if self.over {
            return false;
        }
        for rec in records {
            let cost = rec.len() + 4;
            if self.used + cost > self.limit {
                self.over = true;
                return false;
            }
            self.used += cost;
            self.records.push(rec);
        }
        true
    }

    pub fn frames_fit(&self) -> usize {
        self.records.len()
    }

    pub fn into_records(self) -> Vec<Vec<u8>> {
        self.records
    }
}

pub fn encode_stream(
    frames: impl Iterator<Item = Result<Frame>>,
    fps: u32,
    params: &Params,
    budget: usize,
) -> Result<Streamed> {
    let batch_gops = rayon::current_num_threads().max(1);
    let mut splitter = GopSplitter::new(fps);
    let mut budget = Budget::new(budget);
    let mut batch: Vec<Vec<Frame>> = Vec::new();
    for frame in frames {
        if let Some(gop) = splitter.push(frame?) {
            batch.push(gop);
            if batch.len() >= batch_gops && !flush(&mut batch, params, &mut budget) {
                return Ok(Streamed::Over { frames_fit: budget.frames_fit() });
            }
        }
    }
    batch.extend(splitter.finish());
    if !flush(&mut batch, params, &mut budget) {
        return Ok(Streamed::Over { frames_fit: budget.frames_fit() });
    }
    Ok(Streamed::Fit(budget.into_records()))
}

fn flush(batch: &mut Vec<Vec<Frame>>, params: &Params, budget: &mut Budget) -> bool {
    let encoded: Vec<Vec<Vec<u8>>> = batch.par_iter().map(|g| encode_gop(g, params)).collect();
    batch.clear();
    encoded.into_iter().all(|records| budget.accept(records))
}

pub fn encode_gop(frames: &[Frame], params: &Params) -> Vec<Vec<u8>> {
    let pal = Palette::build(&sample_pixels(frames));
    let pal_rgb: Vec<[f32; 3]> = (0..=255u8).map(|i| pal.rgb(i).map(|v| v as f32)).collect();
    let blocks = (frames[0].w / 2) * (frames[0].h / 2);
    let mut sources = [vec![[0f32; 12]; blocks], vec![[0f32; 12]; blocks]];
    let mut out = Vec::with_capacity(frames.len());
    for (i, frame) in frames.iter().enumerate() {
        let key = i < 2;
        let targets = block_vectors(frame);
        let source = &mut sources[i % 2];
        let update: Vec<usize> =
            (0..blocks).filter(|&b| key || dist(&targets[b], &source[b]) > params.skip as f32).collect();
        let training: Vec<Vec12> = update.iter().map(|&b| targets[b]).collect();
        let codebook = build_codebook(&training, params.codebook, &pal);
        let entries: Vec<Vec12> = codebook.iter().map(|e| entry_vec(e, &pal_rgb)).collect();
        let mut assigned = vec![None; blocks];
        for &b in &update {
            assigned[b] = Some(nearest(&entries, &targets[b]) as u8);
            source[b] = targets[b];
        }
        out.push(write_record(i == 0, key, &pal, &codebook, &assigned));
    }
    out
}

fn sample_pixels(frames: &[Frame]) -> Vec<[u8; 3]> {
    let total: usize = frames.iter().map(|f| f.w * f.h).sum();
    let step = (total / 50_000).max(1);
    frames.iter().flat_map(|f| f.rgb.chunks_exact(3)).step_by(step).map(|c| [c[0], c[1], c[2]]).collect()
}

fn block_vectors(f: &Frame) -> Vec<Vec12> {
    let (bw, bh) = (f.w / 2, f.h / 2);
    let mut out = Vec::with_capacity(bw * bh);
    for by in 0..bh {
        for bx in 0..bw {
            let mut v = [0f32; 12];
            for (p, (dx, dy)) in [(0, 0), (1, 0), (0, 1), (1, 1)].into_iter().enumerate() {
                let i = ((by * 2 + dy) * f.w + bx * 2 + dx) * 3;
                for c in 0..3 {
                    v[p * 3 + c] = f.rgb[i + c] as f32;
                }
            }
            out.push(v);
        }
    }
    out
}

fn entry_vec(e: &[u8; 4], pal_rgb: &[[f32; 3]]) -> Vec12 {
    let mut v = [0f32; 12];
    for p in 0..4 {
        v[p * 3..p * 3 + 3].copy_from_slice(&pal_rgb[e[p] as usize]);
    }
    v
}

fn build_codebook(training: &[Vec12], k: usize, pal: &Palette) -> Vec<[u8; 4]> {
    let step = (training.len() / 4096).max(1);
    let sample: Vec<Vec12> = training.iter().step_by(step).copied().collect();
    let mut entries: Vec<[u8; 4]> = kmeans(&sample, k, 4)
        .iter()
        .map(|c| {
            let mut e = [0u8; 4];
            for p in 0..4 {
                let px = [c[p * 3], c[p * 3 + 1], c[p * 3 + 2]].map(|v| v.round().clamp(0.0, 255.0) as u8);
                e[p] = pal.index(px);
            }
            e
        })
        .collect();
    entries.sort_unstable();
    entries.dedup();
    entries
}

fn write_record(first: bool, key: bool, pal: &Palette, codebook: &[[u8; 4]], assigned: &[Option<u8>]) -> Vec<u8> {
    let flags = key as u8 | if first { 2 } else { 0 };
    let mut out = vec![flags, 0];
    out.extend((codebook.len() as u16).to_le_bytes());
    if first {
        for c in pal.colors {
            out.extend(c.to_le_bytes());
        }
    }
    for e in codebook {
        out.extend(e);
    }
    let mut b = 0;
    while b < assigned.len() {
        let literal = assigned[b].is_some();
        let mut n = 1;
        while n < 128 && b + n < assigned.len() && assigned[b + n].is_some() == literal {
            n += 1;
        }
        if literal {
            out.push(0x7F + n as u8);
            out.extend(assigned[b..b + n].iter().map(|a| a.unwrap()));
        } else {
            out.push(n as u8 - 1);
        }
        b += n;
    }
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::Decoder;
    use crate::frame::Layout;

    fn solid(w: usize, h: usize, c: [u8; 3]) -> Frame {
        Frame { w, h, rgb: c.repeat(w * h) }
    }

    fn gradient(w: usize, h: usize, shift: usize) -> Frame {
        let mut rgb = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                rgb.extend([((x + shift) * 4 % 256) as u8, (y * 5 % 256) as u8, ((x + y) * 2 % 256) as u8]);
            }
        }
        Frame { w, h, rgb }
    }

    fn psnr(a: &[u8], b: &[u8]) -> f64 {
        let mse: f64 = a.iter().zip(b).map(|(&x, &y)| (x as f64 - y as f64).powi(2)).sum::<f64>() / a.len() as f64;
        10.0 * (255.0f64 * 255.0 / mse.max(1e-9)).log10()
    }

    fn layout(w: usize, h: usize) -> Layout {
        Layout { half_res: false, w, h, x_off: 0, y_off: 0 }
    }

    #[test]
    fn static_frames_are_all_skip() {
        let frames = vec![gradient(32, 16, 0); 4];
        let recs = encode_gop(&frames, &Params { codebook: 256, skip: 600 });
        assert_eq!(recs[0][0], 3);
        assert_eq!(recs[1][0], 1);
        assert_eq!(recs[2], vec![0, 0, 0, 0, 0x7F, 0, 0, 0]);
        assert_eq!(recs[3], vec![0, 0, 0, 0, 0x7F, 0, 0, 0]);
    }

    #[test]
    fn records_are_aligned_and_decode() {
        let frames: Vec<Frame> = (0..6).map(|i| gradient(64, 48, i * 3)).collect();
        let recs = encode_gop(&frames, &Params { codebook: 256, skip: 600 });
        let mut d = Decoder::new(layout(64, 48));
        for (rec, frame) in recs.iter().zip(&frames) {
            assert_eq!(rec.len() % 4, 0);
            d.decode(rec);
            let q = psnr(&d.active_rgb(), &frame.rgb);
            assert!(q > 25.0, "psnr {q}");
        }
    }

    #[test]
    fn single_frame_gop() {
        let frames = vec![solid(16, 16, [10, 200, 30])];
        let recs = encode_gop(&frames, &Params { codebook: 64, skip: 3000 });
        assert_eq!(recs.len(), 1);
        let mut d = Decoder::new(layout(16, 16));
        d.decode(&recs[0]);
        assert!(psnr(&d.active_rgb(), &frames[0].rgb) > 30.0);
    }

    #[test]
    fn long_literal_and_skip_runs_split_at_128() {
        let frames = vec![gradient(240, 160, 0), gradient(240, 160, 0), solid(240, 160, [0, 0, 0])];
        let recs = encode_gop(&frames, &Params { codebook: 256, skip: 600 });
        let mut d = Decoder::new(layout(240, 160));
        for r in &recs {
            d.decode(r);
        }
        assert!(psnr(&d.active_rgb(), &frames[2].rgb) > 30.0);
    }

    fn gop_starts(recs: &[Vec<u8>]) -> Vec<usize> {
        recs.iter().enumerate().filter(|(_, r)| r[0] & 2 != 0).map(|(i, _)| i).collect()
    }

    fn fit(s: Streamed) -> Vec<Vec<u8>> {
        match s {
            Streamed::Fit(r) => r,
            Streamed::Over { .. } => panic!("unexpectedly over budget"),
        }
    }

    #[test]
    fn splits_on_scene_cuts_and_length() {
        let p = Params { codebook: 16, skip: 600 };
        let mut frames = vec![solid(8, 8, [0, 0, 0]); 3];
        frames.extend(vec![solid(8, 8, [255, 255, 255]); 3]);
        assert_eq!(gop_starts(&fit(encode_stream(frames.into_iter().map(Ok), 25, &p, usize::MAX).unwrap())), vec![0, 3]);
        assert!(fit(encode_stream(std::iter::empty(), 1, &p, usize::MAX).unwrap()).is_empty());
    }

    #[test]
    fn stream_stops_reading_once_over_budget() {
        let consumed = std::cell::Cell::new(0);
        let frames = (0..1000).map(|i| {
            consumed.set(consumed.get() + 1);
            Ok(gradient(64, 48, i * 7))
        });
        match encode_stream(frames, 5, &Params { codebook: 256, skip: 0 }, 20_000).unwrap() {
            Streamed::Over { frames_fit } => assert!(frames_fit < consumed.get()),
            Streamed::Fit(_) => panic!("should be over budget"),
        }
        assert!(consumed.get() < 1000, "read {} frames", consumed.get());
    }

    #[test]
    fn stream_counts_bytes_against_budget() {
        let frames: Vec<Frame> = (0..8).map(|i| gradient(32, 16, i * 5)).collect();
        let p = Params { codebook: 64, skip: 600 };
        let all = fit(encode_stream(frames.clone().into_iter().map(Ok), 5, &p, usize::MAX).unwrap());
        let total: usize = all.iter().map(|r| r.len() + 4).sum();
        assert_eq!(fit(encode_stream(frames.clone().into_iter().map(Ok), 5, &p, total).unwrap()), all);
        assert!(matches!(
            encode_stream(frames.into_iter().map(Ok), 5, &p, total - 1).unwrap(),
            Streamed::Over { frames_fit: 7 }
        ));
    }

    #[test]
    fn stream_propagates_decode_errors() {
        let frames = vec![Ok(solid(8, 8, [0, 0, 0])), Err(anyhow::anyhow!("boom"))];
        assert!(encode_stream(frames.into_iter(), 5, &Params { codebook: 16, skip: 600 }, usize::MAX).is_err());
    }

    #[test]
    fn stream_keeps_order() {
        let mut frames: Vec<Frame> = (0..5).map(|_| solid(8, 8, [0, 0, 0])).collect();
        frames.extend((0..5).map(|_| solid(8, 8, [255, 255, 255])));
        let recs = fit(encode_stream(frames.into_iter().map(Ok), 20, &Params { codebook: 16, skip: 600 }, usize::MAX).unwrap());
        assert_eq!(recs.len(), 10);
        assert_eq!(recs[0][0], 3);
        assert_eq!(recs[5][0], 3);
        let mut d = Decoder::new(layout(8, 8));
        for r in &recs {
            d.decode(r);
        }
        assert_eq!(&d.active_rgb()[..3], &[255, 255, 255]);
    }

    #[test]
    fn push_api_matches_encode_stream() {
        let mut frames: Vec<Frame> = (0..30).map(|i| gradient(32, 16, i)).collect();
        frames.extend((0..10).map(|_| solid(32, 16, [255, 255, 255])));
        let p = Params { codebook: 64, skip: 600 };
        let expected = fit(encode_stream(frames.clone().into_iter().map(Ok), 5, &p, usize::MAX).unwrap());
        let mut splitter = GopSplitter::new(5);
        let mut budget = Budget::new(usize::MAX);
        for f in frames {
            if let Some(g) = splitter.push(f) {
                assert!(budget.accept(encode_gop(&g, &p)));
            }
        }
        if let Some(g) = splitter.finish() {
            assert!(budget.accept(encode_gop(&g, &p)));
        }
        assert_eq!(budget.frames_fit(), 40);
        assert_eq!(budget.into_records(), expected);
    }

    #[test]
    fn budget_rejects_once_over_and_stays_over() {
        let mut b = Budget::new(20);
        assert!(b.accept(vec![vec![0; 8], vec![0; 4]]));
        assert!(!b.accept(vec![vec![0; 4]]));
        assert_eq!(b.frames_fit(), 2);
        assert!(!b.accept(vec![]));
        assert_eq!(b.into_records().len(), 2);
    }

    #[test]
    fn empty_stream_fits_with_no_records() {
        assert!(GopSplitter::new(5).finish().is_none());
        assert!(fit(encode_stream(std::iter::empty(), 5, &Params { codebook: 16, skip: 600 }, 0).unwrap()).is_empty());
    }

    #[test]
    fn gop_length_is_two_seconds() {
        let p = Params { codebook: 16, skip: 600 };
        let same = vec![solid(8, 8, [9, 9, 9]); 10];
        assert_eq!(gop_starts(&fit(encode_stream(same.into_iter().map(Ok), 2, &p, usize::MAX).unwrap())), vec![0, 4, 8]);
    }
}
