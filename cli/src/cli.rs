use clap::Parser;
use std::path::PathBuf;
use vid2gba_core::time::parse_time;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum TargetArg {
    Emulator,
    Gba,
}

#[derive(Parser, Debug)]
#[command(name = "vid2gba", version, about = "Convert a video file into a Game Boy Advance ROM")]
pub struct Cli {
    #[arg(required = true, num_args = 1.., help = "Video files to convert (several make a menu cartridge)")]
    pub input: Vec<PathBuf>,
    #[arg(short, long, help = "Output .gba path (default: <title>.gba)")]
    pub output: Option<PathBuf>,
    #[arg(long, value_parser = parse_time, help = "Start time (SS, MM:SS or HH:MM:SS)")]
    pub start: Option<f64>,
    #[arg(long, value_parser = parse_time, help = "End time (SS, MM:SS or HH:MM:SS)")]
    pub end: Option<f64>,
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..=30), help = "Pin the frame rate")]
    pub fps: Option<u32>,
    #[arg(long, help = "Force 120x80 half resolution")]
    pub half_res: bool,
    #[arg(long, default_value_t = 32 * 1024 * 1024, value_parser = clap::value_parser!(u64).range(1..=32 * 1024 * 1024), help = "Maximum ROM size in bytes (at most 32 MiB)")]
    pub max_size: u64,
    #[arg(long, help = "Cartridge title")]
    pub title: Option<String>,
    #[arg(long, value_enum, default_value_t = TargetArg::Emulator, help = "Screen the cartridge is made for")]
    pub target: TargetArg,
    #[arg(long, help = "Subtitle file (.srt or .vtt)")]
    pub subtitles: Option<PathBuf>,
    #[arg(long, help = "Ignore subtitles, including embedded ones")]
    pub no_subtitles: bool,
    #[arg(long, help = "Font file for subtitles in other scripts")]
    pub subtitle_font: Option<PathBuf>,
}
