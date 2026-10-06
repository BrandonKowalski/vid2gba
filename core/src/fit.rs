use anyhow::Result;

pub const SKIP_LOW: u32 = 600;
pub const SKIP_MEDIUM: u32 = 1500;
pub const SKIP_HIGH: u32 = 3000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Step {
    pub half_res: bool,
    pub fps: u32,
    pub codebook: usize,
    pub skip: u32,
}

const fn step(half_res: bool, fps: u32, codebook: usize, skip: u32) -> Step {
    Step { half_res, fps, codebook, skip }
}

pub const LADDER: [Step; 7] = [
    step(false, 20, 256, SKIP_LOW),
    step(false, 15, 256, SKIP_LOW),
    step(false, 15, 128, SKIP_MEDIUM),
    step(false, 12, 128, SKIP_HIGH),
    step(false, 10, 64, SKIP_HIGH),
    step(true, 15, 256, SKIP_LOW),
    step(true, 10, 128, SKIP_HIGH),
];

pub fn candidates(fps: Option<u32>, half_res: bool) -> Vec<Step> {
    let mut out: Vec<Step> = Vec::new();
    for s in LADDER {
        if half_res && !s.half_res {
            continue;
        }
        let s = Step { fps: fps.unwrap_or(s.fps), ..s };
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

pub enum Attempt<T> {
    Fit(T),
    Over { secs: f64 },
}

pub enum Outcome<T> {
    Fit(Step, T),
    TooBig { best_secs: f64 },
}

pub fn first_fit<T>(steps: &[Step], mut encode: impl FnMut(&Step) -> Result<Attempt<T>>) -> Result<Outcome<T>> {
    let mut best_secs = 0.0f64;
    for s in steps {
        match encode(s)? {
            Attempt::Fit(v) => return Ok(Outcome::Fit(*s, v)),
            Attempt::Over { secs } => best_secs = best_secs.max(secs),
        }
    }
    Ok(Outcome::TooBig { best_secs })
}

fn mib(bytes: usize) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

pub fn audio_too_big_message(secs: f64, max: usize, fixed: usize, audio_len: usize, with_subtitles: bool, hint: &str) -> String {
    let audio_rate = audio_len as f64 / secs.max(0.001);
    let fits = (max as f64 - (fixed - audio_len) as f64).max(0.0) / audio_rate;
    let what = if with_subtitles { "the audio and subtitles alone need" } else { "the audio alone needs" };
    format!(
        "the clip ({secs:.0}s) doesn't fit in {:.1} MiB: {what} {:.1} MiB; roughly {fits:.0}s would fit. {hint}",
        mib(max),
        mib(audio_len)
    )
}

pub fn too_big_message(secs: f64, max: usize, best_secs: f64, hint: &str) -> String {
    format!(
        "the clip ({secs:.0}s) doesn't fit in {:.1} MiB even at the lowest quality; roughly {best_secs:.0}s would fit. {hint}",
        mib(max)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ladder_order_is_best_first() {
        assert_eq!(candidates(None, false), LADDER.to_vec());
        assert!(candidates(None, true).iter().all(|s| s.half_res));
        assert_eq!(candidates(None, true).len(), 2);
    }

    #[test]
    fn pinned_fps_dedupes() {
        let c = candidates(Some(24), false);
        assert!(c.iter().all(|s| s.fps == 24));
        let unique: std::collections::HashSet<_> = c.iter().map(|s| (s.half_res, s.codebook, s.skip)).collect();
        assert_eq!(unique.len(), c.len());
    }

    #[test]
    fn picks_first_that_fits() {
        let mut tried = 0;
        let out = first_fit(&LADDER, |_| {
            tried += 1;
            Ok(if tried < 3 { Attempt::Over { secs: 1.0 } } else { Attempt::Fit(tried) })
        })
        .unwrap();
        assert!(matches!(out, Outcome::Fit(s, 3) if s == LADDER[2]));
    }

    #[test]
    fn reports_longest_fitting_duration_when_nothing_fits() {
        let out = first_fit(&LADDER, |s| Ok(Attempt::<()>::Over { secs: 100.0 / s.fps as f64 })).unwrap();
        assert!(matches!(out, Outcome::TooBig { best_secs } if best_secs == 10.0));
    }

    #[test]
    fn messages_include_hint_and_estimates() {
        let m = too_big_message(600.0, 32 * 1024 * 1024, 412.4, "HINT");
        assert!(m.contains("600s") && m.contains("32.0 MiB") && m.contains("412s") && m.contains("HINT"), "{m}");
        let a = audio_too_big_message(6000.0, 32 * 1024 * 1024, 40 * 1024 * 1024, 39 * 1024 * 1024, false, "HINT");
        assert!(a.contains("audio alone") && a.contains("HINT"), "{a}");
    }

    #[test]
    fn audio_message_mentions_subtitles() {
        assert!(audio_too_big_message(10.0, 1000, 2000, 1500, true, "H").contains("audio and subtitles alone need"));
        assert!(audio_too_big_message(10.0, 1000, 2000, 1500, false, "H").contains("the audio alone needs"));
    }
}
