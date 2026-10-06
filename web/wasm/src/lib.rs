use vid2gba_core::codec::{Budget, GopSplitter, Params, encode_gop};
use vid2gba_core::container::{self, Video};
use vid2gba_core::dvdmenu::{self, MenuEntry};
use vid2gba_core::frame::{Frame, Layout};
use vid2gba_core::resample::Resampler;
use vid2gba_core::subs::{self, Cue, CueImage, SubCue};
use vid2gba_core::tone::{self, Target};
use vid2gba_core::{adpcm, fit, rom, time};
use wasm_bindgen::prelude::*;

const HINT: &str = "Set a start and end to pick a shorter segment.";
const OUT_RATE: f64 = container::AUDIO_RATE as f64;

fn err(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

#[wasm_bindgen]
pub fn parse_time(s: &str) -> Result<f64, JsError> {
    time::parse_time(s).map_err(err)
}

#[wasm_bindgen]
pub fn validate_range(start: Option<f64>, end: Option<f64>, duration: f64) -> Result<(), JsError> {
    time::validate(start, end, duration).map_err(err)
}

#[wasm_bindgen]
#[derive(Clone, Copy)]
pub struct JsLayout {
    pub half_res: bool,
    pub w: u32,
    pub h: u32,
    pub x_off: u32,
    pub y_off: u32,
}

impl JsLayout {
    fn core(&self) -> Layout {
        Layout { half_res: self.half_res, w: self.w as usize, h: self.h as usize, x_off: self.x_off as usize, y_off: self.y_off as usize }
    }
}

#[wasm_bindgen]
pub fn fit_layout(src_w: u32, src_h: u32, half_res: bool) -> JsLayout {
    let l = Layout::fit(src_w, src_h, half_res);
    JsLayout { half_res: l.half_res, w: l.w as u32, h: l.h as u32, x_off: l.x_off as u32, y_off: l.y_off as u32 }
}

#[wasm_bindgen]
#[derive(Clone, Copy)]
pub struct JsStep {
    pub half_res: bool,
    pub fps: u32,
    pub codebook: u32,
    pub skip: u32,
}

#[wasm_bindgen]
pub fn candidates(fps: Option<u32>, half_res: bool) -> Vec<JsStep> {
    fit::candidates(fps, half_res)
        .into_iter()
        .map(|s| JsStep { half_res: s.half_res, fps: s.fps, codebook: s.codebook as u32, skip: s.skip })
        .collect()
}

#[wasm_bindgen]
pub struct AudioEncoder {
    resampler: Resampler,
    pcm: Vec<i16>,
}

#[wasm_bindgen]
impl AudioEncoder {
    #[wasm_bindgen(constructor)]
    pub fn new(in_rate: f64) -> AudioEncoder {
        AudioEncoder { resampler: Resampler::new(in_rate, OUT_RATE), pcm: Vec::new() }
    }

    pub fn push(&mut self, mono: &[f32]) {
        let out = self.resampler.push(mono);
        self.pcm.extend(out);
    }

    pub fn push_silence(&mut self, samples: u32) {
        self.pcm.extend(std::iter::repeat_n(0i16, samples as usize));
    }

    pub fn finish(&mut self) -> Vec<u8> {
        let tail = self.resampler.finish();
        self.pcm.extend(tail);
        adpcm::encode(&self.pcm)
    }

    pub fn samples(&self) -> u32 {
        self.pcm.len() as u32
    }
}

fn pack_frames(frames: Vec<Frame>) -> Vec<u8> {
    frames.into_iter().flat_map(|f| f.rgb).collect()
}

#[wasm_bindgen]
pub struct Splitter {
    inner: GopSplitter,
    w: usize,
    h: usize,
    target: Target,
}

#[wasm_bindgen]
impl Splitter {
    #[wasm_bindgen(constructor)]
    pub fn new(fps: u32, w: u32, h: u32, real_gba: bool) -> Splitter {
        Splitter {
            inner: GopSplitter::new(fps),
            w: w as usize,
            h: h as usize,
            target: if real_gba { Target::Gba } else { Target::Emulator },
        }
    }

    pub fn push(&mut self, rgba: &[u8]) -> Option<Vec<u8>> {
        let mut rgb = Vec::with_capacity(self.w * self.h * 3);
        for px in rgba.chunks_exact(4).take(self.w * self.h) {
            rgb.extend_from_slice(&px[..3]);
        }
        let mut frame = Frame { w: self.w, h: self.h, rgb };
        tone::apply(&mut frame, self.target);
        self.inner.push(frame).map(pack_frames)
    }

    pub fn finish(&mut self) -> Option<Vec<u8>> {
        self.inner.finish().map(pack_frames)
    }
}

fn pack_records(records: &[Vec<u8>]) -> Vec<u8> {
    let mut out = (records.len() as u32).to_le_bytes().to_vec();
    for r in records {
        out.extend((r.len() as u32).to_le_bytes());
        out.extend_from_slice(r);
    }
    out
}

fn unpack_records(data: &[u8]) -> Vec<Vec<u8>> {
    let n = u32::from_le_bytes(data[..4].try_into().unwrap()) as usize;
    let mut p = 4;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let len = u32::from_le_bytes(data[p..p + 4].try_into().unwrap()) as usize;
        out.push(data[p + 4..p + 4 + len].to_vec());
        p += 4 + len;
    }
    out
}

#[wasm_bindgen]
pub fn encode_gop_packed(gop: &[u8], w: u32, h: u32, codebook: u32, skip: u32) -> Vec<u8> {
    let (w, h) = (w as usize, h as usize);
    let frames: Vec<Frame> = gop.chunks_exact(w * h * 3).map(|c| Frame { w, h, rgb: c.to_vec() }).collect();
    pack_records(&encode_gop(&frames, &Params { codebook: codebook as usize, skip }))
}

struct Part {
    src: (u32, u32),
    audio: Vec<u8>,
    samples: u32,
    subtitles: Vec<SubCue>,
    layout: Option<(Layout, u32)>,
}

#[wasm_bindgen]
pub struct CartBuilder {
    parts: Vec<Part>,
    entries: Vec<MenuEntry>,
    budget: Option<Budget>,
    bounds: Vec<usize>,
}

impl Default for CartBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl CartBuilder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> CartBuilder {
        CartBuilder { parts: Vec::new(), entries: Vec::new(), budget: None, bounds: Vec::new() }
    }

    pub fn add_video(&mut self, src_w: u32, src_h: u32, audio: &[u8], samples: u32, subtitles: &Subtitles) {
        self.parts.push(Part { src: (src_w, src_h), audio: audio.to_vec(), samples, subtitles: subtitles.sub_cues(), layout: None });
    }

    pub fn add_menu_entry(&mut self, title: &str, duration: f64, rgba: &[u8]) {
        let rgb: Vec<u8> = rgba.chunks_exact(4).take(dvdmenu::THUMB_W * dvdmenu::THUMB_H).flat_map(|p| [p[0], p[1], p[2]]).collect();
        self.entries.push(MenuEntry { title: title.to_string(), duration_secs: duration, thumbnail: Frame { w: dvdmenu::THUMB_W, h: dvdmenu::THUMB_H, rgb } });
    }

    fn pages(&self, title: &str) -> Vec<dvdmenu::MenuPage> {
        if self.parts.len() > 1 { dvdmenu::render_pages(title, &self.entries) } else { Vec::new() }
    }

    pub fn fixed_size(&self, title: &str) -> Result<u32, JsError> {
        let videos: Vec<Video> = self
            .parts
            .iter()
            .map(|p| Video { layout: Layout::fit(p.src.0, p.src.1, false), fps: 1, frames: &[], audio: &p.audio, audio_samples: p.samples, title, subtitles: &p.subtitles })
            .collect();
        let data = container::build_set(title, &videos, &self.pages(title));
        Ok(rom::assemble(rom::PLAYER, &data, title).map_err(err)?.len() as u32)
    }

    pub fn begin_step(&mut self, limit: u32) {
        self.budget = Some(Budget::new(limit as usize));
        self.bounds.clear();
        for p in &mut self.parts {
            p.layout = None;
        }
    }

    pub fn accept(&mut self, packed: &[u8]) -> bool {
        self.budget.as_mut().is_some_and(|b| b.accept(unpack_records(packed)))
    }

    pub fn end_video(&mut self, layout: &JsLayout, fps: u32) {
        let i = self.bounds.len();
        self.bounds.push(self.frames_fit() as usize);
        if let Some(p) = self.parts.get_mut(i) {
            p.layout = Some((layout.core(), fps));
        }
    }

    pub fn frames_fit(&self) -> u32 {
        self.budget.as_ref().map_or(0, |b| b.frames_fit() as u32)
    }

    pub fn build(&mut self, title: &str) -> Result<Vec<u8>, JsError> {
        let records = self.budget.take().ok_or_else(|| err("no frames"))?.into_records();
        let mut start = 0;
        let mut slices = Vec::new();
        for &end in &self.bounds {
            slices.push(records[start..end].to_vec());
            start = end;
        }
        let videos: Vec<Video> = self
            .parts
            .iter()
            .zip(&slices)
            .map(|(p, frames)| {
                let (layout, fps) = p.layout.unwrap_or((Layout::fit(p.src.0, p.src.1, false), 1));
                Video { layout, fps, frames, audio: &p.audio, audio_samples: p.samples, title, subtitles: &p.subtitles }
            })
            .collect();
        let data = container::build_set(title, &videos, &self.pages(title));
        rom::assemble(rom::PLAYER, &data, title).map_err(err)
    }
}

#[wasm_bindgen]
pub fn max_rom_size() -> u32 {
    32 * 1024 * 1024
}

#[wasm_bindgen]
pub fn audio_too_big_message(secs: f64, max: u32, fixed: u32, audio_len: u32, with_subtitles: bool) -> String {
    fit::audio_too_big_message(secs, max as usize, fixed as usize, audio_len as usize, with_subtitles, HINT)
}

#[wasm_bindgen]
pub fn too_big_message(secs: f64, max: u32, best_secs: f64) -> String {
    fit::too_big_message(secs, max as usize, best_secs, HINT)
}

#[wasm_bindgen]
pub struct Subtitles {
    cues: Vec<Cue>,
    images: Vec<Option<CueImage>>,
}

#[wasm_bindgen]
impl Subtitles {
    #[wasm_bindgen(constructor)]
    pub fn new(text: &str, start: f64, end: f64) -> Result<Subtitles, JsError> {
        let cues = subs::trim(&subs::parse(text).map_err(err)?, start, end);
        let images = vec![None; cues.len()];
        Ok(Subtitles { cues, images })
    }

    pub fn empty() -> Subtitles {
        Subtitles { cues: Vec::new(), images: Vec::new() }
    }

    pub fn count(&self) -> u32 {
        self.cues.len() as u32
    }

    pub fn text(&self, i: u32) -> String {
        self.cues[i as usize].lines.join("\n")
    }

    pub fn set_image(&mut self, i: u32, alpha: &[u8], w: u32, h: u32) {
        self.images[i as usize] = Some(subs::pack(alpha, w as usize, h as usize));
    }
}

impl Subtitles {
    fn sub_cues(&self) -> Vec<SubCue> {
        self.cues
            .iter()
            .zip(&self.images)
            .filter_map(|(c, img)| img.clone().map(|image| SubCue { start: c.start, end: c.end, image }))
            .collect()
    }
}
