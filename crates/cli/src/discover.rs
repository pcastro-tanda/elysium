//! Turns CLI path arguments into the list of files to lint.
//!
//! Semantics mirror RuboCop's `TargetFinder`:
//! - a directory is walked and filtered through `AllCops/Include`/`Exclude`;
//! - a file given explicitly is linted whether or not it matches `Include`;
//! - a glob is expanded relative to the project root and filtered like a
//!   directory walk;
//! - `.gitignore` is honoured unless disabled, which RuboCop does through its
//!   `.rubocop.yml` `Exclude` instead; honouring it by default is the
//!   behaviour Rails teams expect from a native tool.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use config::FileMatcher;
use globset::{Glob, GlobMatcher};
use ignore::{WalkBuilder, WalkState};
use parking_lot::Mutex;

/// Discovery options.
#[derive(Debug, Clone)]
pub struct Options<'a> {
    /// Directory that relative patterns are resolved against.
    pub root: &'a Path,
    /// Include/Exclude filter.
    pub matcher: &'a FileMatcher,
    /// Honour `.gitignore` and friends.
    pub gitignore: bool,
}

/// Expands `inputs` into a sorted, de-duplicated list of absolute target
/// files. Inputs are resolved against `opts.root` first so that
/// `Include`/`Exclude` patterns, which are root-relative, match regardless of
/// whether the user wrote `.`, `app/`, or an absolute path.
pub fn discover(inputs: &[PathBuf], opts: &Options<'_>) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if inputs.is_empty() {
        walk(opts.root, None, opts, &mut files)?;
    }
    for input in inputs {
        if is_glob(input) {
            let pattern = input.to_str().context("glob pattern is not valid UTF-8")?;
            let matcher = Glob::new(pattern)
                .with_context(|| format!("invalid glob `{pattern}`"))?
                .compile_matcher();
            walk(opts.root, Some(&matcher), opts, &mut files)?;
            continue;
        }
        let input = normalize(opts.root, input);
        if input.is_dir() {
            walk(&input, None, opts, &mut files)?;
        } else if input.is_file() {
            files.push(input);
        } else {
            anyhow::bail!("no such file or directory: {}", input.display());
        }
    }

    files.sort();
    files.dedup();
    Ok(files)
}

/// Joins onto `root` and removes `.` and `..` components lexically. No
/// filesystem access, so symlinks are left alone and the result stays a
/// prefix-comparable sibling of `root`.
fn normalize(root: &Path, input: &Path) -> PathBuf {
    use std::path::Component;

    let joined = if input.is_absolute() { input.to_path_buf() } else { root.join(input) };
    let mut out = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

fn is_glob(path: &Path) -> bool {
    path.to_str().is_some_and(|s| s.contains(['*', '?', '[', '{']))
}

fn walk(
    dir: &Path,
    glob: Option<&GlobMatcher>,
    opts: &Options<'_>,
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    let mut builder = WalkBuilder::new(dir);
    builder
        .hidden(false)
        .git_ignore(opts.gitignore)
        .git_global(opts.gitignore)
        .git_exclude(opts.gitignore)
        .ignore(false)
        .parents(opts.gitignore)
        .require_git(false)
        .follow_links(false);

    // The walk is I/O bound (one readdir per directory plus .gitignore
    // parsing); `ignore`'s parallel walker overlaps those syscalls. One
    // uncontended lock per matched file is negligible next to the readdir.
    let found = Mutex::new(Vec::new());
    let first_error: Mutex<Option<ignore::Error>> = Mutex::new(None);
    builder.build_parallel().run(|| {
        Box::new(|entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    first_error.lock().get_or_insert(err);
                    return WalkState::Continue;
                }
            };
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                return WalkState::Continue;
            }
            let path = entry.path();
            let relative = path.strip_prefix(opts.root).unwrap_or(path);
            if let Some(glob) = glob {
                if !glob.is_match(relative) {
                    return WalkState::Continue;
                }
            }
            if !opts.matcher.is_excluded(relative)
                && (opts.matcher.is_included(relative) || (glob.is_none() && ruby_shebang(path)))
            {
                found.lock().push(path.to_path_buf());
            }
            WalkState::Continue
        })
    });
    if let Some(err) = first_error.into_inner() {
        return Err(err.into());
    }
    out.append(&mut found.into_inner());
    Ok(())
}

/// RuboCop also lints extensionless files whose first line is a Ruby
/// shebang (`#!/usr/bin/env ruby`). Only consulted for extensionless files
/// that matched nothing else, so the extra read is rare.
fn ruby_shebang(path: &Path) -> bool {
    use std::io::Read as _;

    if path.extension().is_some() {
        return false;
    }
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    let mut buf = [0u8; 128];
    let Ok(n) = file.read(&mut buf) else { return false };
    let head = &buf[..n];
    if !head.starts_with(b"#!") {
        return false;
    }
    let line = head.split(|&b| b == b'\n').next().unwrap_or(head);
    let Ok(line) = std::str::from_utf8(line) else { return false };
    ["ruby", "macruby", "rake", "jruby", "rbx"]
        .iter()
        .any(|interp| line.split(['/', ' ']).any(|word| word == *interp))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parallel_walk_reports_every_file() {
        // 300 files: not a multiple of any plausible batch size, so a lost
        // per-worker tail would show up as a short count.
        let dir = std::env::temp_dir().join(format!("elysium-discover-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for i in 0..300 {
            let sub = dir.join(format!("d{}", i % 7));
            std::fs::create_dir_all(&sub).unwrap();
            std::fs::write(sub.join(format!("f{i}.rb")), b"1\n").unwrap();
        }
        std::fs::write(dir.join("notes.md"), b"x").unwrap();
        std::fs::write(dir.join(".gitignore"), b"d3/\n").unwrap();
        std::fs::create_dir_all(dir.join("vendor/bin")).unwrap();
        std::fs::write(dir.join("vendor/bin/tool"), b"#!/usr/bin/env ruby\n1\n").unwrap();
        std::fs::create_dir_all(dir.join("bin")).unwrap();
        std::fs::write(dir.join("bin/tool"), b"#!/usr/bin/env ruby\n1\n").unwrap();

        let matcher = FileMatcher::rubocop_defaults();
        let opts = Options { root: &dir, matcher: &matcher, gitignore: false };
        let all = discover(std::slice::from_ref(&dir), &opts).unwrap();
        assert_eq!(all.len(), 301);
        assert!(all.windows(2).all(|w| w[0] < w[1]), "sorted and deduplicated");

        let opts = Options { root: &dir, matcher: &matcher, gitignore: true };
        let ignored = discover(std::slice::from_ref(&dir), &opts).unwrap();
        let in_d3 = (0..300).filter(|i| i % 7 == 3).count();
        assert_eq!(ignored.len(), 300 - in_d3 + 1);

        let globbed = discover(&[PathBuf::from("d1/*.rb")], &opts).unwrap();
        assert_eq!(globbed.len(), (0..300).filter(|i| i % 7 == 1).count());

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
