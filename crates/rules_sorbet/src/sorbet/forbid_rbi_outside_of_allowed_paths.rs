//! `Sorbet/ForbidRBIOutsideOfAllowedPaths`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/forbid_rbi_outside_of_allowed_paths.rb`.

use linter::OptionValue;
use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::Node;

/// Forbids RBI files outside of the allowed paths
#[derive(Debug, Clone)]
pub struct ForbidRBIOutsideOfAllowedPaths {
    /// `cop_config["AllowedPaths"]` when it is an array, `nil` entries dropped.
    paths: Option<Vec<String>>,
}

impl Rule for ForbidRBIOutsideOfAllowedPaths {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/ForbidRBIOutsideOfAllowedPaths",
        department: Department::Sorbet,
        summary: "Forbids RBI files outside of the allowed paths",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[ConfigOption {
            name: "AllowedPaths",
            default: ConfigDefault::StrList(&["rbi/**", "sorbet/rbi/**"]),
            allowed: &[],
            doc: "A list of the paths where RBI files are allowed.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let paths = match options.get("AllowedPaths") {
            Some(OptionValue::List(items)) => Some(
                items
                    .iter()
                    .filter_map(|item| match item {
                        OptionValue::Null => None,
                        other => Some(other.as_str().map_or_else(String::new, str::to_owned)),
                    })
                    .collect(),
            ),
            Some(_) => None,
            // A configured cop always carries `Enabled`; the merge with the
            // default config drops a key set to `~`, so absence there is `nil`.
            None if options.get("Enabled").is_some() => None,
            None => Some(options.str_list("AllowedPaths")),
        };
        Ok(Self { paths })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let Some(paths) = &self.paths else {
            ctx.report_global(&Self::META, "AllowedPaths expects an array");
            return;
        };
        if paths.is_empty() {
            ctx.report_global(&Self::META, "AllowedPaths cannot be empty");
            return;
        }
        // When executed the path to the source file is absolute; remove the
        // exec path directory prefix before matching (`String#sub`, first hit).
        let file_path = ctx.source().path().to_string_lossy().into_owned();
        let rel_path = match std::env::current_dir() {
            Ok(pwd) => {
                let prefix = format!("{}/", pwd.to_string_lossy());
                file_path.replacen(&prefix, "", 1)
            }
            Err(_) => file_path,
        };
        if !paths.iter().any(|pattern| fnmatch(pattern.as_bytes(), rel_path.as_bytes())) {
            ctx.report_global(
                &Self::META,
                format!("RBI file path should match one of: {}", paths.join(", ")),
            );
        }
    }

    fn enter(&mut self, _node: &Node<'_>, _ctx: &mut Context<'_>) {}
}

/// `File.fnmatch(pattern, path)` with no flags: `*` and `?` match any
/// character (including `/`), but not a leading `.`; `[...]` classes and
/// backslash escapes are supported; `{}` alternation is not.
fn fnmatch(pattern: &[u8], path: &[u8]) -> bool {
    fn matches(pattern: &[u8], path: &[u8], at_start: bool) -> bool {
        let Some((&first, rest)) = pattern.split_first() else { return path.is_empty() };
        match first {
            b'*' => {
                let mut rest = rest;
                while let [b'*', tail @ ..] = rest {
                    rest = tail;
                }
                if at_start && path.first() == Some(&b'.') {
                    return false;
                }
                (0..=path.len()).any(|skip| matches(rest, &path[skip..], false))
            }
            b'?' => {
                !(path.is_empty() || at_start && path[0] == b'.')
                    && matches(rest, &path[char_len(path)..], false)
            }
            b'[' => {
                let Some(&ch) = path.first() else { return false };
                if at_start && ch == b'.' {
                    return false;
                }
                match class_match(rest, &path[..char_len(path)]) {
                    Some((true, after)) => matches(after, &path[char_len(path)..], false),
                    Some((false, _)) => false,
                    // No closing `]`: a literal `[`.
                    None => ch == b'[' && matches(rest, &path[1..], false),
                }
            }
            b'\\' if !rest.is_empty() => {
                path.first() == Some(&rest[0]) && matches(&rest[1..], &path[1..], false)
            }
            _ => path.first() == Some(&first) && matches(rest, &path[1..], false),
        }
    }

    fn char_len(path: &[u8]) -> usize {
        match path[0] {
            b if b < 0x80 => 1,
            b if b >= 0xF0 => 4.min(path.len()),
            b if b >= 0xE0 => 3.min(path.len()),
            _ => 2.min(path.len()),
        }
    }

    /// Matches `ch` against a bracket expression starting after `[`; returns
    /// whether it matched and the pattern after the closing `]`.
    fn class_match<'a>(pattern: &'a [u8], ch: &[u8]) -> Option<(bool, &'a [u8])> {
        let (negate, mut rest) = match pattern.first() {
            Some(b'!' | b'^') => (true, &pattern[1..]),
            _ => (false, pattern),
        };
        let mut matched = false;
        let mut first = true;
        loop {
            let (&c, tail) = rest.split_first()?;
            if c == b']' && !first {
                return Some((matched != negate, tail));
            }
            first = false;
            let (lo, tail) =
                if c == b'\\' && !tail.is_empty() { (tail[0], &tail[1..]) } else { (c, tail) };
            if tail.first() == Some(&b'-') && tail.len() >= 2 && tail[1] != b']' {
                let hi = tail[1];
                if ch.len() == 1 && lo <= ch[0] && ch[0] <= hi {
                    matched = true;
                }
                rest = &tail[2..];
            } else {
                if ch == [lo] {
                    matched = true;
                }
                rest = tail;
            }
        }
    }

    matches(pattern, path, true)
}
