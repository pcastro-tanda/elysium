//! Parses the `--stats` line `elysium check` prints to stderr, e.g.
//! `files: 32448 (109465081 bytes)  nodes: 11403411  discover: 281.9ms  \
//! lint: 715.6ms  output: 6.5ms  total: 1.0s  threads: 10`.

use anyhow::{anyhow, bail, Context as _, Result};

/// The subset of one `--stats` line this benchmark cares about.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RunStats {
    /// `files` count.
    pub(crate) files: f64,
    /// `discover` phase duration, in milliseconds.
    pub(crate) discover_ms: f64,
    /// `lint` phase duration, in milliseconds.
    pub(crate) lint_ms: f64,
    /// `total` wall-clock duration, in milliseconds.
    pub(crate) total_ms: f64,
}

/// Finds and parses the `--stats` line among `elysium`'s stderr output.
pub(crate) fn parse_stderr(stderr: &str) -> Result<RunStats> {
    let line = stderr
        .lines()
        .find(|line| line.starts_with("files:"))
        .ok_or_else(|| anyhow!("no `--stats` line found in stderr:\n{stderr}"))?;
    parse_stats_line(line)
}

/// Parses one `--stats` line, splitting on the double spaces between fields
/// and the `": "` between each field's name and value.
fn parse_stats_line(line: &str) -> Result<RunStats> {
    let mut files = None;
    let mut discover_ms = None;
    let mut lint_ms = None;
    let mut total_ms = None;

    for field in line.split("  ").map(str::trim).filter(|f| !f.is_empty()) {
        let (name, value) = field
            .split_once(": ")
            .ok_or_else(|| anyhow!("malformed `--stats` field {field:?} in line {line:?}"))?;
        match name {
            "files" => {
                let count = value.split(" (").next().unwrap_or(value).trim();
                files = Some(
                    count
                        .parse::<f64>()
                        .with_context(|| format!("parsing file count {count:?}"))?,
                );
            }
            "discover" => discover_ms = Some(parse_duration_ms(value)?),
            "lint" => lint_ms = Some(parse_duration_ms(value)?),
            "total" => total_ms = Some(parse_duration_ms(value)?),
            _ => {}
        }
    }

    Ok(RunStats {
        files: files.ok_or_else(|| anyhow!("missing `files` field in {line:?}"))?,
        discover_ms: discover_ms.ok_or_else(|| anyhow!("missing `discover` field in {line:?}"))?,
        lint_ms: lint_ms.ok_or_else(|| anyhow!("missing `lint` field in {line:?}"))?,
        total_ms: total_ms.ok_or_else(|| anyhow!("missing `total` field in {line:?}"))?,
    })
}

/// Parses a Rust `{:.1?}`-formatted [`std::time::Duration`] (e.g. `658.6ms`,
/// `1.4s`, `12.0µs`, `45.0ns`) into milliseconds.
fn parse_duration_ms(value: &str) -> Result<f64> {
    let value = value.trim();
    if let Some(n) = value.strip_suffix("µs") {
        Ok(n.trim().parse::<f64>()? / 1_000.0)
    } else if let Some(n) = value.strip_suffix("ms") {
        Ok(n.trim().parse::<f64>()?)
    } else if let Some(n) = value.strip_suffix("ns") {
        Ok(n.trim().parse::<f64>()? / 1_000_000.0)
    } else if let Some(n) = value.strip_suffix('s') {
        Ok(n.trim().parse::<f64>()? * 1_000.0)
    } else {
        bail!("unrecognized duration suffix in {value:?}")
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::{parse_duration_ms, parse_stderr};

    #[test]
    fn parses_a_real_stats_line() {
        let stderr = "files: 32448 (109465081 bytes)  nodes: 11403411  discover: 281.9ms  lint: 715.6ms  output: 6.5ms  total: 1.0s  threads: 10\n";
        let stats = parse_stderr(stderr).unwrap();
        assert_eq!(stats.files, 32448.0);
        assert!((stats.discover_ms - 281.9).abs() < 1e-9);
        assert!((stats.lint_ms - 715.6).abs() < 1e-9);
        assert!((stats.total_ms - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn parses_all_duration_suffixes() {
        assert!((parse_duration_ms("45.0ns").unwrap() - 0.000_045).abs() < 1e-9);
        assert!((parse_duration_ms("12.0µs").unwrap() - 0.012).abs() < 1e-9);
        assert!((parse_duration_ms("658.6ms").unwrap() - 658.6).abs() < 1e-9);
        assert!((parse_duration_ms("1.4s").unwrap() - 1400.0).abs() < 1e-9);
    }

    #[test]
    fn rejects_missing_stats_line() {
        assert!(parse_stderr("nothing to see here\n").is_err());
    }
}
