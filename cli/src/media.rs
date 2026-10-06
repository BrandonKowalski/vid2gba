use vid2gba_core::container::AUDIO_RATE;
use vid2gba_core::frame::Frame;
use anyhow::{Context, Result, bail, ensure};
use std::path::{Path, PathBuf};
use std::io::Read;
use std::process::{Child, ChildStdout, Command, Stdio};

#[derive(Debug)]
pub struct Source {
    pub path: PathBuf,
    pub title: String,
}

#[derive(Clone, Debug)]
pub struct Probe {
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub has_audio: bool,
    pub subtitles: Vec<(usize, String)>,
}

pub fn check_tool(tool: &str, version_flag: &str) -> Result<()> {
    match Command::new(tool).arg(version_flag).stdout(Stdio::null()).stderr(Stdio::null()).status() {
        Ok(_) => Ok(()),
        Err(_) => bail!("{tool} not found on PATH; install ffmpeg (https://ffmpeg.org/download.html)"),
    }
}

fn run(cmd: &mut Command) -> Result<Vec<u8>> {
    let name = cmd.get_program().to_string_lossy().into_owned();
    let out = cmd.stdin(Stdio::null()).output().with_context(|| format!("failed to run {name}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let lines: Vec<&str> = err.lines().collect();
        bail!("{name} failed:\n{}", lines[lines.len().saturating_sub(20)..].join("\n"));
    }
    Ok(out.stdout)
}

pub fn open(path: &Path) -> Result<Source> {
    ensure!(path.is_file(), "file not found: {}", path.display());
    let title = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(Source { path: path.to_path_buf(), title })
}

pub fn parse_probe(json: &[u8]) -> Result<Probe> {
    let v: serde_json::Value = serde_json::from_slice(json)?;
    let streams = v["streams"].as_array().context("ffprobe returned no streams")?;
    let video = streams.iter().find(|s| s["codec_type"] == "video").context("input has no video stream")?;
    let mut width = video["width"].as_u64().context("video width missing")?;
    let mut height = video["height"].as_u64().context("video height missing")?;
    if let Some((a, b)) = video["sample_aspect_ratio"].as_str().and_then(|s| s.split_once(':')) {
        if let (Ok(a), Ok(b)) = (a.parse::<u64>(), b.parse::<u64>()) {
            if a > 0 && b > 0 {
                width = width * a / b;
            }
        }
    }
    let rotation = video["side_data_list"]
        .as_array()
        .into_iter()
        .flatten()
        .find_map(|d| d["rotation"].as_f64())
        .or_else(|| video["tags"]["rotate"].as_str().and_then(|r| r.parse().ok()))
        .unwrap_or(0.0);
    if (rotation.abs().round() as i64) % 180 == 90 {
        std::mem::swap(&mut width, &mut height);
    }
    let duration = v["format"]["duration"]
        .as_str()
        .and_then(|d| d.parse::<f64>().ok())
        .context("video duration missing")?;
    Ok(Probe {
        width: width as u32,
        height: height as u32,
        duration,
        has_audio: streams.iter().any(|s| s["codec_type"] == "audio"),
        subtitles: streams
            .iter()
            .filter(|s| s["codec_type"] == "subtitle")
            .enumerate()
            .map(|(i, s)| (i, s["codec_name"].as_str().unwrap_or("unknown").to_string()))
            .collect(),
    })
}

pub fn probe(path: &Path) -> Result<Probe> {
    parse_probe(&run(Command::new("ffprobe").args(["-v", "error", "-show_streams", "-show_format", "-of", "json"]).arg(path))?)
}

pub enum SubtitleChoice {
    Text(usize),
    ImageOnly(String),
    None,
}

const TEXT_CODECS: [&str; 6] = ["subrip", "srt", "ass", "ssa", "webvtt", "mov_text"];

pub fn pick_subtitles(probe: &Probe) -> SubtitleChoice {
    if let Some((i, _)) = probe.subtitles.iter().find(|(_, c)| TEXT_CODECS.contains(&c.as_str())) {
        return SubtitleChoice::Text(*i);
    }
    match probe.subtitles.first() {
        Some((_, c)) => SubtitleChoice::ImageOnly(c.clone()),
        None => SubtitleChoice::None,
    }
}

pub fn extract_subtitles(path: &Path, n: usize) -> Result<String> {
    let out = run(Command::new("ffmpeg").args(["-v", "error", "-nostdin", "-i"]).arg(path).args(["-map", &format!("0:s:{n}"), "-f", "srt", "-"]))?;
    String::from_utf8(out).context("embedded subtitles are not valid UTF-8")
}

pub fn thumbnail(path: &Path, at: f64, w: usize, h: usize) -> Result<Frame> {
    let vf = format!("scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2");
    let out = run(Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin", "-ss", &format!("{at}"), "-i"])
        .arg(path)
        .args(["-frames:v", "1", "-vf", &vf, "-pix_fmt", "rgb24", "-f", "rawvideo", "-"]))?;
    ensure!(out.len() >= w * h * 3, "could not grab a thumbnail from {}", path.display());
    Ok(Frame { w, h, rgb: out[..w * h * 3].to_vec() })
}

pub fn clip_duration(total: f64, start: Option<f64>, end: Option<f64>) -> f64 {
    (end.unwrap_or(total).min(total) - start.unwrap_or(0.0)).max(0.0)
}

fn trim_args(start: Option<f64>, end: Option<f64>) -> Vec<String> {
    let mut a = Vec::new();
    if let Some(s) = start {
        a.extend(["-ss".to_string(), format!("{s}")]);
    }
    if let Some(e) = end {
        a.extend(["-to".to_string(), format!("{e}")]);
    }
    a
}

pub struct VideoStream {
    child: Child,
    stdout: ChildStdout,
    w: usize,
    h: usize,
    done: bool,
}

pub fn stream_video(path: &Path, start: Option<f64>, end: Option<f64>, fps: u32, w: usize, h: usize) -> Result<VideoStream> {
    let mut child = Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin"])
        .args(trim_args(start, end))
        .arg("-i")
        .arg(path)
        .args(["-an", "-vf", &format!("fps={fps},scale={w}:{h}:flags=area,setsar=1")])
        .args(["-pix_fmt", "rgb24", "-f", "rawvideo", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to run ffmpeg")?;
    let stdout = child.stdout.take().context("ffmpeg stdout unavailable")?;
    Ok(VideoStream { child, stdout, w, h, done: false })
}

impl VideoStream {
    fn read_frame(&mut self) -> std::io::Result<Option<Frame>> {
        let mut buf = vec![0u8; self.w * self.h * 3];
        let mut filled = 0;
        while filled < buf.len() {
            let n = self.stdout.read(&mut buf[filled..])?;
            if n == 0 {
                return Ok(None);
            }
            filled += n;
        }
        Ok(Some(Frame { w: self.w, h: self.h, rgb: buf }))
    }

    fn finish(&mut self) -> Result<()> {
        let mut err = String::new();
        if let Some(mut stderr) = self.child.stderr.take() {
            stderr.read_to_string(&mut err).ok();
        }
        if !self.child.wait()?.success() {
            let lines: Vec<&str> = err.lines().collect();
            bail!("ffmpeg failed:\n{}", lines[lines.len().saturating_sub(20)..].join("\n"));
        }
        Ok(())
    }
}

impl Iterator for VideoStream {
    type Item = Result<Frame>;

    fn next(&mut self) -> Option<Result<Frame>> {
        if self.done {
            return None;
        }
        match self.read_frame() {
            Ok(Some(frame)) => Some(Ok(frame)),
            Ok(None) => {
                self.done = true;
                self.finish().err().map(Err)
            }
            Err(e) => {
                self.done = true;
                Some(Err(e.into()))
            }
        }
    }
}

impl Drop for VideoStream {
    fn drop(&mut self) {
        if !self.done {
            self.child.kill().ok();
            self.child.wait().ok();
        }
    }
}

#[cfg(test)]
pub fn decode_video(path: &Path, start: Option<f64>, end: Option<f64>, fps: u32, w: usize, h: usize) -> Result<Vec<Frame>> {
    stream_video(path, start, end, fps, w, h)?.collect()
}

pub fn decode_audio(path: &Path, probe: &Probe, start: Option<f64>, end: Option<f64>) -> Result<Vec<i16>> {
    if !probe.has_audio {
        let secs = clip_duration(probe.duration, start, end);
        return Ok(vec![0; (secs * AUDIO_RATE as f64) as usize]);
    }
    let out = run(Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin"])
        .args(trim_args(start, end))
        .arg("-i")
        .arg(path)
        .args(["-vn", "-ac", "1", "-ar", &AUDIO_RATE.to_string(), "-f", "s16le", "-"]))?;
    Ok(out.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(dir: &Path, audio: bool) -> PathBuf {
        let path = dir.join(if audio { "a.mkv" } else { "v.mkv" });
        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc=size=320x180:rate=30:duration=2"]);
        if audio {
            cmd.args(["-f", "lavfi", "-i", "sine=frequency=440:duration=2", "-c:a", "pcm_s16le"]);
        }
        cmd.args(["-c:v", "mpeg4", "-q:v", "4"]).arg(&path);
        assert!(cmd.status().unwrap().success());
        path
    }

    #[test]
    fn probe_applies_rotation_and_sar() {
        let rotated = br#"{"streams":[{"codec_type":"video","width":1920,"height":1080,"sample_aspect_ratio":"1:1","side_data_list":[{"side_data_type":"Display Matrix","rotation":-90}]}],"format":{"duration":"12.5"}}"#;
        let p = parse_probe(rotated).unwrap();
        assert_eq!((p.width, p.height, p.has_audio), (1080, 1920, false));
        assert_eq!(p.duration, 12.5);
        let anamorphic = br#"{"streams":[{"codec_type":"video","width":720,"height":480,"sample_aspect_ratio":"4:3"},{"codec_type":"audio"}],"format":{"duration":"1"}}"#;
        let p = parse_probe(anamorphic).unwrap();
        assert_eq!((p.width, p.height, p.has_audio), (960, 480, true));
        assert!(parse_probe(br#"{"streams":[{"codec_type":"audio"}],"format":{"duration":"1"}}"#).is_err());
    }

    #[test]
    fn clip_duration_clamps() {
        assert_eq!(clip_duration(10.0, None, None), 10.0);
        assert_eq!(clip_duration(10.0, Some(2.0), Some(50.0)), 8.0);
        assert_eq!(clip_duration(10.0, Some(2.0), Some(5.0)), 3.0);
    }

    #[test]
    fn probes_and_decodes_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = clip(dir.path(), true);
        let p = probe(&path).unwrap();
        assert_eq!((p.width, p.height), (320, 180));
        assert!(p.has_audio && (p.duration - 2.0).abs() < 0.1);
        let frames = decode_video(&path, None, None, 10, 240, 134).unwrap();
        assert!((19..=21).contains(&frames.len()), "{}", frames.len());
        assert!(frames.iter().all(|f| f.w == 240 && f.h == 134 && f.rgb.len() == 240 * 134 * 3));
        let trimmed = decode_video(&path, Some(0.5), Some(1.5), 10, 240, 134).unwrap();
        assert!((9..=11).contains(&trimmed.len()), "{}", trimmed.len());
        let pcm = decode_audio(&path, &p, None, None).unwrap();
        assert!((pcm.len() as i64 - 2 * 13379).abs() < 400, "{}", pcm.len());
        assert!(pcm.iter().any(|&s| s.abs() > 1000));
    }

    #[test]
    fn stream_yields_frames_then_ends() {
        let dir = tempfile::tempdir().unwrap();
        let path = clip(dir.path(), false);
        let mut s = stream_video(&path, None, None, 10, 240, 134).unwrap();
        assert_eq!(s.next().unwrap().unwrap().rgb.len(), 240 * 134 * 3);
        let rest: Vec<Result<Frame>> = s.collect();
        assert!(rest.iter().all(|f| f.is_ok()));
        assert!((18..=20).contains(&rest.len()), "{}", rest.len());
    }

    #[test]
    fn stream_reports_ffmpeg_failure() {
        let dir = tempfile::tempdir().unwrap();
        let bad = dir.path().join("bad.mkv");
        std::fs::write(&bad, b"not a video").unwrap();
        let results: Vec<Result<Frame>> = stream_video(&bad, None, None, 10, 240, 134).unwrap().collect();
        let err = results.last().expect("no result").as_ref().unwrap_err().to_string();
        assert!(err.contains("ffmpeg failed"), "{err}");
    }

    #[test]
    fn silent_source_decodes_to_zeros() {
        let dir = tempfile::tempdir().unwrap();
        let path = clip(dir.path(), false);
        let p = probe(&path).unwrap();
        assert!(!p.has_audio);
        let pcm = decode_audio(&path, &p, Some(0.5), None).unwrap();
        assert!((pcm.len() as i64 - (1.5 * 13379.0) as i64).abs() < 400);
        assert!(pcm.iter().all(|&s| s == 0));
    }

    #[test]
    fn open_uses_file_stem_as_title() {
        let dir = tempfile::tempdir().unwrap();
        let path = clip(dir.path(), false);
        let s = open(&path).unwrap();
        assert_eq!(s.path, path);
        assert_eq!(s.title, "v");
    }

    #[test]
    fn open_reports_missing_file() {
        let e = open(Path::new("/nonexistent/x.mp4")).unwrap_err().to_string();
        assert!(e.contains("file not found: /nonexistent/x.mp4"), "{e}");
        let dir = tempfile::tempdir().unwrap();
        let e = open(dir.path()).unwrap_err().to_string();
        assert!(e.contains("file not found"), "{e}");
    }

    #[test]
    fn missing_tool_is_reported() {
        let e = check_tool("definitely-not-a-tool-vid2gba", "--version").unwrap_err().to_string();
        assert!(e.contains("not found on PATH") && e.contains("ffmpeg.org"), "{e}");
        assert!(!e.contains("brew"), "{e}");
    }

    #[test]
    fn picks_text_subtitles_and_reports_image_only() {
        let text = br#"{"streams":[{"codec_type":"video","width":320,"height":180},{"codec_type":"subtitle","codec_name":"hdmv_pgs_subtitle"},{"codec_type":"subtitle","codec_name":"subrip"}],"format":{"duration":"1"}}"#;
        let p = parse_probe(text).unwrap();
        assert_eq!(p.subtitles, vec![(0, "hdmv_pgs_subtitle".to_string()), (1, "subrip".to_string())]);
        assert!(matches!(pick_subtitles(&p), SubtitleChoice::Text(1)));
        let image = br#"{"streams":[{"codec_type":"video","width":320,"height":180},{"codec_type":"subtitle","codec_name":"dvd_subtitle"}],"format":{"duration":"1"}}"#;
        assert!(matches!(pick_subtitles(&parse_probe(image).unwrap()), SubtitleChoice::ImageOnly(c) if c == "dvd_subtitle"));
    }

    #[test]
    fn grabs_letterboxed_thumbnail() {
        let dir = tempfile::tempdir().unwrap();
        let path = clip(dir.path(), false);
        let t = thumbnail(&path, 0.5, 64, 40).unwrap();
        assert_eq!((t.w, t.h, t.rgb.len()), (64, 40, 64 * 40 * 3));
        assert!(t.rgb.iter().any(|&v| v > 100));
    }
}
