mod cli;
mod media;
mod subtext;

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use cli::{Cli, TargetArg};
use std::path::{Path, PathBuf};
use vid2gba_core::codec::Streamed;
use vid2gba_core::fit::{Attempt, Outcome};
use vid2gba_core::frame::Layout;
use vid2gba_core::{adpcm, codec, container, dvdmenu, fit, rom, subs, time, tone};

const HINT: &str = "Use --start/--end to pick a shorter segment.";

fn main() -> Result<()> {
    run(Cli::parse())
}

fn default_output(title: &str) -> PathBuf {
    let name: String = title.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    let name = name.trim_matches('_');
    PathBuf::from(format!("{}.gba", if name.is_empty() { "video" } else { name }))
}

fn mib(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

struct Prepared {
    path: PathBuf,
    probe: media::Probe,
    title: String,
    start: Option<f64>,
    end: Option<f64>,
    secs: f64,
    samples: u32,
    audio: Vec<u8>,
    subtitles: Vec<subs::SubCue>,
}

fn load_cues(cli: &Cli, single: bool, path: &Path, probe: &media::Probe, start: f64, end: f64) -> Result<Vec<subs::Cue>> {
    if cli.no_subtitles {
        return Ok(Vec::new());
    }
    if single {
        if let Some(p) = &cli.subtitles {
            let bytes = std::fs::read(p).with_context(|| format!("cannot read {}", p.display()))?;
            let text = String::from_utf8(bytes).map_err(|_| anyhow::anyhow!("{} is not UTF-8 text; re-save it as UTF-8", p.display()))?;
            let cues = subs::trim(&subs::parse(&text).context("subtitles")?, start, end);
            if cues.is_empty() {
                eprintln!("notice: no subtitle lines in the selected range");
            }
            return Ok(cues);
        }
    }
    Ok(match media::pick_subtitles(probe) {
        media::SubtitleChoice::Text(n) => match media::extract_subtitles(path, n).and_then(|t| subs::parse(&t)) {
            Ok(all) => {
                let cues = subs::trim(&all, start, end);
                if cues.is_empty() {
                    eprintln!("notice: no subtitle lines in the selected range");
                }
                cues
            }
            Err(e) => {
                eprintln!("notice: embedded subtitles skipped: {e}");
                Vec::new()
            }
        },
        media::SubtitleChoice::ImageOnly(codec) => {
            eprintln!("subtitles skipped: image-based subtitles ({codec}) are not supported");
            Vec::new()
        }
        media::SubtitleChoice::None => Vec::new(),
    })
}

fn prepare(path: &Path, cli: &Cli, single: bool, renderer: &mut subtext::Renderer) -> Result<Prepared> {
    let source = media::open(path)?;
    let probe = media::probe(&source.path)?;
    let (start, end) = if single { (cli.start, cli.end) } else { (None, None) };
    time::validate(start, end, probe.duration).map_err(anyhow::Error::msg)?;
    eprintln!("decoding audio: {}", path.display());
    let pcm = media::decode_audio(&source.path, &probe, start, end)?;
    let clip_start = start.unwrap_or(0.0);
    let clip_end = end.map_or(probe.duration, |e| e.min(probe.duration));
    let cues = load_cues(cli, single, &source.path, &probe, clip_start, clip_end)?;
    let subtitles = cues.iter().map(|c| subs::SubCue { start: c.start, end: c.end, image: renderer.render(c) }).collect();
    Ok(Prepared {
        secs: media::clip_duration(probe.duration, start, end),
        samples: pcm.len() as u32,
        audio: adpcm::encode(&pcm),
        title: source.title,
        path: source.path,
        probe,
        start,
        end,
        subtitles,
    })
}

fn run(cli: Cli) -> Result<()> {
    let max_size = cli.max_size as usize;
    media::check_tool("ffmpeg", "-version")?;
    media::check_tool("ffprobe", "-version")?;
    let single = cli.input.len() == 1;
    ensure!(
        single || (cli.start.is_none() && cli.end.is_none() && cli.subtitles.is_none()),
        "--start/--end/--subtitles work with a single input; use the web app for per-video trimming"
    );
    time::validate(cli.start, cli.end, f64::INFINITY).map_err(anyhow::Error::msg)?;
    let target = match cli.target {
        TargetArg::Emulator => tone::Target::Emulator,
        TargetArg::Gba => tone::Target::Gba,
    };
    let mut renderer = match &cli.subtitle_font {
        Some(p) => subtext::Renderer::from_file(p)?,
        None => subtext::Renderer::bundled(),
    };
    let videos: Vec<Prepared> = cli.input.iter().map(|p| prepare(p, &cli, single, &mut renderer)).collect::<Result<_>>()?;
    if renderer.missing() > 0 {
        eprintln!("subtitles: {} characters not in the font; use --subtitle-font", renderer.missing());
    }
    let cart_title = cli.title.clone().unwrap_or_else(|| if single { videos[0].title.clone() } else { "vid2gba".to_string() });
    let pages = if single {
        Vec::new()
    } else {
        let entries = videos
            .iter()
            .map(|v| {
                let at = v.start.unwrap_or(0.0) + v.secs * 0.1;
                Ok(dvdmenu::MenuEntry {
                    title: v.title.clone(),
                    duration_secs: v.secs,
                    thumbnail: media::thumbnail(&v.path, at, dvdmenu::THUMB_W, dvdmenu::THUMB_H)?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        dvdmenu::render_pages(&cart_title, &entries)
    };
    let total_secs: f64 = videos.iter().map(|v| v.secs).sum();
    let audio_total: usize = videos.iter().map(|v| v.audio.len()).sum();
    let has_subs = videos.iter().any(|v| !v.subtitles.is_empty());
    let empty: Vec<container::Video> = videos
        .iter()
        .map(|v| container::Video {
            layout: Layout::fit(v.probe.width, v.probe.height, false),
            fps: 1,
            frames: &[],
            audio: &v.audio,
            audio_samples: v.samples,
            title: &cart_title,
            subtitles: &v.subtitles,
        })
        .collect();
    let fixed = rom::assemble(rom::PLAYER, &container::build_set(&cart_title, &empty, &pages), &cart_title)?.len();
    if fixed > max_size {
        bail!("{}", fit::audio_too_big_message(total_secs, max_size, fixed, audio_total, has_subs, HINT));
    }
    let budget = max_size - fixed;
    let steps = fit::candidates(cli.fps, cli.half_res);
    let outcome = fit::first_fit(&steps, |step| {
        let params = codec::Params { codebook: step.codebook, skip: step.skip };
        let mut used = 0usize;
        let mut done = 0usize;
        let mut encoded: Vec<(Layout, Vec<Vec<u8>>)> = Vec::new();
        for v in &videos {
            let layout = Layout::fit(v.probe.width, v.probe.height, step.half_res);
            eprintln!(
                "decoding video: {} {}x{} @ {} fps; encoding: codebook {}, skip threshold {}",
                v.path.display(),
                layout.w,
                layout.h,
                step.fps,
                step.codebook,
                step.skip
            );
            let frames = media::stream_video(&v.path, v.start, v.end, step.fps, layout.w, layout.h)?.map(|f| {
                f.map(|mut f| {
                    tone::apply(&mut f, target);
                    f
                })
            });
            match codec::encode_stream(frames, step.fps, &params, budget - used)? {
                Streamed::Over { frames_fit } => {
                    let fit_secs = (done + frames_fit) as f64 / step.fps as f64;
                    eprintln!("  over budget after {fit_secs:.0}s of video");
                    return Ok(Attempt::Over { secs: fit_secs });
                }
                Streamed::Fit(records) => {
                    ensure!(!records.is_empty(), "no video frames were decoded from {}", v.path.display());
                    used += records.iter().map(|r| r.len() + 4).sum::<usize>();
                    done += records.len();
                    encoded.push((layout, records));
                }
            }
        }
        let parts: Vec<container::Video> = videos
            .iter()
            .zip(&encoded)
            .map(|(v, (layout, records))| container::Video {
                layout: *layout,
                fps: step.fps,
                frames: records,
                audio: &v.audio,
                audio_samples: v.samples,
                title: &cart_title,
                subtitles: &v.subtitles,
            })
            .collect();
        let image = rom::assemble(rom::PLAYER, &container::build_set(&cart_title, &parts, &pages), &cart_title)?;
        ensure!(image.len() <= max_size, "internal error: ROM exceeds the size budget");
        eprintln!("  {:.1} MiB", mib(image.len()));
        let summary: Vec<(Layout, usize)> = encoded.iter().map(|(l, r)| (*l, r.len())).collect();
        Ok(Attempt::Fit((image, summary)))
    })?;
    let (step, (image, summary)) = match outcome {
        Outcome::Fit(step, v) => (step, v),
        Outcome::TooBig { best_secs } => bail!("{}", fit::too_big_message(total_secs, max_size, best_secs, HINT)),
    };
    let out = cli.output.clone().unwrap_or_else(|| default_output(&cart_title));
    std::fs::write(&out, &image)?;
    if single {
        let (layout, count) = summary[0];
        eprintln!(
            "wrote {} ({:.1} MiB): {}x{}{} @ {} fps, {} frames",
            out.display(),
            mib(image.len()),
            layout.w,
            layout.h,
            if layout.half_res { " (half-res)" } else { "" },
            step.fps,
            count
        );
    } else {
        eprintln!("wrote {} ({:.1} MiB): {} videos @ {} fps", out.display(), mib(image.len()), summary.len(), step.fps);
    }
    Ok(())
}
