pub fn parse_time(s: &str) -> Result<f64, String> {
    let invalid = || format!("invalid time '{s}' (use SS, MM:SS or HH:MM:SS)");
    let parts: Vec<&str> = s.trim().split(':').collect();
    if parts.len() > 3 {
        return Err(invalid());
    }
    let mut total = 0.0;
    for (i, part) in parts.iter().enumerate() {
        let v: f64 = part.parse().map_err(|_| invalid())?;
        let last = i == parts.len() - 1;
        if !v.is_finite() || v < 0.0 || (i > 0 && v >= 60.0) || (!last && v.fract() != 0.0) {
            return Err(invalid());
        }
        total = total * 60.0 + v;
    }
    Ok(total)
}

pub fn validate(start: Option<f64>, end: Option<f64>, duration: f64) -> Result<(), String> {
    if let (Some(s), Some(e)) = (start, end) {
        if e <= s {
            return Err(format!("--end ({e}s) must be after --start ({s}s)"));
        }
    }
    if let Some(s) = start {
        if s >= duration {
            return Err(format!("--start ({s}s) is past the end of the video ({duration:.1}s)"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_seconds() {
        assert_eq!(parse_time("90").unwrap(), 90.0);
        assert_eq!(parse_time("1.5").unwrap(), 1.5);
    }

    #[test]
    fn parses_minutes_and_hours() {
        assert_eq!(parse_time("1:23").unwrap(), 83.0);
        assert_eq!(parse_time("1:02:03.5").unwrap(), 3723.5);
    }

    #[test]
    fn rejects_garbage() {
        for s in ["", "abc", "1:75", "-3", "1:2:3:4", "1.5:00", "nan"] {
            assert!(parse_time(s).is_err(), "{s}");
        }
    }

    #[test]
    fn validates_range() {
        assert!(validate(Some(10.0), Some(5.0), 100.0).is_err());
        assert!(validate(Some(10.0), Some(10.0), 100.0).is_err());
        assert!(validate(Some(100.0), None, 100.0).is_err());
        assert!(validate(Some(10.0), Some(500.0), 100.0).is_ok());
        assert!(validate(None, None, 100.0).is_ok());
        assert!(validate(Some(5.0), Some(9.0), f64::INFINITY).is_ok());
    }
}
