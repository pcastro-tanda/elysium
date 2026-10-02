//! `RubyGems`' `Gem::Version` and `Gem::Requirement`, as far as
//! `requires_gem` needs them: parsing the versions a lockfile holds and
//! requirement strings (`">= 3.1.0"`, `"~> 7.0"`), and comparing them.

use std::cmp::Ordering;

/// One segment of a version: `Gem::Version` splits `"1.0.0.rc1"` into
/// `1, 0, 0, "rc", 1`.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Number(u64),
    Text(String),
}

/// `Gem::Version`; equality is `<=>`'s (`"3.1" == "3.1.0"`).
#[derive(Debug, Clone)]
pub struct GemVersion {
    segments: Vec<Segment>,
}

impl GemVersion {
    /// `Gem::Version.new(text)`; `None` for text RubyGems rejects.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = if text.is_empty() { "0" } else { text };
        if !text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-') {
            return None;
        }
        if !text.as_bytes()[0].is_ascii_digit() {
            return None;
        }
        // `-` stands for `.pre.`: `"1.0-1"` is `"1.0.pre.1"`.
        let text = text.replace('-', ".pre.");
        let mut segments = Vec::new();
        let mut rest = text.as_str();
        while let Some(start) = rest.find(|c: char| c.is_ascii_alphanumeric()) {
            rest = &rest[start..];
            let digits = rest.starts_with(|c: char| c.is_ascii_digit());
            let end = rest
                .find(|c: char| c.is_ascii_digit() != digits || !c.is_ascii_alphanumeric())
                .unwrap_or(rest.len());
            let (piece, tail) = rest.split_at(end);
            segments.push(if digits {
                Segment::Number(piece.parse().ok()?)
            } else {
                Segment::Text(piece.to_string())
            });
            rest = tail;
        }
        Some(Self { segments })
    }

    /// `#canonical_segments`: trailing zeros dropped from the numeric part
    /// and from the prerelease part.
    fn canonical(&self) -> Vec<&Segment> {
        let split = self
            .segments
            .iter()
            .position(|s| matches!(s, Segment::Text(_)))
            .unwrap_or(self.segments.len());
        let (numeric, text) = self.segments.split_at(split);
        let mut out: Vec<&Segment> = Vec::new();
        for part in [numeric, text] {
            let kept = part.iter().rposition(|s| *s != Segment::Number(0)).map_or(0, |i| i + 1);
            out.extend(&part[..kept]);
        }
        out
    }

    /// `#release`: the version without its prerelease part.
    fn release(&self) -> Vec<u64> {
        self.segments
            .iter()
            .map_while(|s| match s {
                Segment::Number(n) => Some(*n),
                Segment::Text(_) => None,
            })
            .collect()
    }
}

impl PartialEq for GemVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for GemVersion {}

impl PartialOrd for GemVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for GemVersion {
    /// `Gem::Version#<=>`.
    fn cmp(&self, other: &Self) -> Ordering {
        let (left, right) = (self.canonical(), other.canonical());
        let zero = Segment::Number(0);
        for i in 0..left.len().max(right.len()) {
            let (l, r) =
                (left.get(i).copied().unwrap_or(&zero), right.get(i).copied().unwrap_or(&zero));
            let order = match (l, r) {
                (Segment::Number(a), Segment::Number(b)) => a.cmp(b),
                (Segment::Text(a), Segment::Text(b)) => a.cmp(b),
                (Segment::Text(_), Segment::Number(_)) => Ordering::Less,
                (Segment::Number(_), Segment::Text(_)) => Ordering::Greater,
            };
            if order != Ordering::Equal {
                return order;
            }
        }
        Ordering::Equal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operator {
    Equal,
    NotEqual,
    Greater,
    Less,
    GreaterEqual,
    LessEqual,
    Pessimistic,
}

/// `Gem::Requirement`: every clause must hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GemRequirement {
    clauses: Vec<(Operator, GemVersion)>,
}

impl GemRequirement {
    /// `Gem::Requirement.new(*requirements)`; with none, any version is
    /// accepted (`">= 0"`). `None` when a clause is malformed.
    pub fn parse(requirements: &[&str]) -> Option<Self> {
        let mut clauses = Vec::new();
        for requirement in requirements {
            let requirement = requirement.trim();
            let (operator, version) = [
                ("!=", Operator::NotEqual),
                (">=", Operator::GreaterEqual),
                ("<=", Operator::LessEqual),
                ("~>", Operator::Pessimistic),
                ("=", Operator::Equal),
                (">", Operator::Greater),
                ("<", Operator::Less),
            ]
            .iter()
            .find_map(|(text, op)| requirement.strip_prefix(text).map(|rest| (*op, rest)))
            .unwrap_or((Operator::Equal, requirement));
            clauses.push((operator, GemVersion::parse(version)?));
        }
        if clauses.is_empty() {
            clauses.push((Operator::GreaterEqual, GemVersion::parse("0")?));
        }
        Some(Self { clauses })
    }

    /// `#satisfied_by?`.
    pub fn satisfied_by(&self, version: &GemVersion) -> bool {
        self.clauses.iter().all(|(operator, required)| match operator {
            Operator::Equal => version == required,
            Operator::NotEqual => version != required,
            Operator::Greater => version > required,
            Operator::Less => version < required,
            Operator::GreaterEqual => version >= required,
            Operator::LessEqual => version <= required,
            Operator::Pessimistic => version >= required && version.release() < bump(required),
        })
    }
}

/// `Gem::Version#bump`, as a release segment list: `"3.1.2"` becomes `3.2`.
fn bump(version: &GemVersion) -> Vec<u64> {
    let mut segments = version.release();
    if segments.len() > 1 {
        segments.pop();
    }
    if let Some(last) = segments.last_mut() {
        *last += 1;
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(text: &str) -> GemVersion {
        GemVersion::parse(text).unwrap()
    }

    fn satisfied(requirements: &[&str], version: &str) -> bool {
        GemRequirement::parse(requirements).unwrap().satisfied_by(&v(version))
    }

    #[test]
    fn orders_like_rubygems() {
        assert!(v("3.1.0") > v("3.0.9"));
        assert!(v("3.10") > v("3.9"));
        assert_eq!(v("3.1"), v("3.1.0"));
        assert!(v("3.1.0.rc1") < v("3.1.0"));
        assert!(v("3.1.0.rc1") < v("3.1.0.rc2"));
        assert!(v("1.0-1") < v("1.0"));
    }

    #[test]
    fn requirements() {
        assert!(satisfied(&[">= 3.1.0"], "3.1.0"));
        assert!(!satisfied(&[">= 3.1.0"], "3.0.0"));
        assert!(satisfied(&["~> 3.1"], "3.9.2"));
        assert!(!satisfied(&["~> 3.1"], "4.0"));
        assert!(!satisfied(&["~> 3.1.2"], "3.2.0"));
        assert!(satisfied(&[">= 1", "< 2"], "1.5"));
        assert!(!satisfied(&[">= 1", "< 2"], "2.0"));
        assert!(satisfied(&[], "0.0.1"));
        assert!(satisfied(&["!= 2.0"], "2.1"));
    }
}
