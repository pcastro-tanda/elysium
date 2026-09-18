//! Lexical path helpers matching the Ruby ones RuboCop relies on
//! (`File.expand_path`, `PathUtil.relative_path`, `Dir.glob`).

use std::path::{Component, Path, PathBuf};

/// `File.expand_path(path, base)`: makes `path` absolute against `base` and
/// resolves `.`/`..` lexically, without touching the file system.
pub(crate) fn expand(path: &Path, base: &Path) -> PathBuf {
    let joined = if path.is_absolute() { path.to_path_buf() } else { base.join(path) };
    normalize(&joined)
}

/// Resolves `.` and `..` components lexically.
pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// `PathUtil.relative_path(path, base)`: a relative path, using `..` when
/// `path` is outside `base`. Falls back to `path` when the two share no root.
pub(crate) fn relative(path: &Path, base: &Path) -> PathBuf {
    let path = normalize(path);
    let base = normalize(base);
    let mut path_parts = path.components().peekable();
    let mut base_parts = base.components().peekable();
    while let (Some(a), Some(b)) = (path_parts.peek(), base_parts.peek()) {
        if a == b {
            path_parts.next();
            base_parts.next();
        } else {
            break;
        }
    }
    let mut out = PathBuf::new();
    for _ in base_parts {
        out.push("..");
    }
    for part in path_parts {
        out.push(part);
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

/// True when the string needs `Dir.glob` (`PathUtil.glob?`).
pub(crate) fn is_glob(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('?') || pattern.contains('[')
}

/// A minimal `Dir.glob`: expands `*`, `?` and `**` path components against the
/// file system, returning existing paths sorted lexically.
pub(crate) fn glob(pattern: &Path) -> Vec<PathBuf> {
    let mut current: Vec<PathBuf> = Vec::new();
    let mut first = true;
    for component in pattern.components() {
        let part = component.as_os_str().to_string_lossy().to_string();
        if first {
            current.push(PathBuf::from(&part));
            first = false;
            continue;
        }
        if part == "**" {
            let mut next = current.clone();
            let mut frontier = current.clone();
            while !frontier.is_empty() {
                let mut deeper = Vec::new();
                for base in &frontier {
                    for entry in read_dir_sorted(base) {
                        if entry.is_dir() {
                            deeper.push(entry);
                        }
                    }
                }
                next.extend(deeper.iter().cloned());
                frontier = deeper;
            }
            current = next;
        } else if is_glob(&part) {
            let mut next = Vec::new();
            for base in &current {
                for entry in read_dir_sorted(base) {
                    let name = entry.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if wildcard_match(&part, &name) {
                        next.push(entry);
                    }
                }
            }
            current = next;
        } else {
            for base in &mut current {
                base.push(&part);
            }
            current.retain(|p| p.exists());
        }
        if current.is_empty() {
            return current;
        }
    }
    current.sort();
    current.dedup();
    current
}

fn read_dir_sorted(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    out.sort();
    out
}

/// `File.fnmatch` for a single path component: `*` and `?` only.
fn wildcard_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    let mut memo = vec![vec![false; n.len() + 1]; p.len() + 1];
    memo[0][0] = true;
    for i in 1..=p.len() {
        if p[i - 1] == '*' {
            memo[i][0] = memo[i - 1][0];
        }
    }
    for i in 1..=p.len() {
        for j in 1..=n.len() {
            memo[i][j] = match p[i - 1] {
                '*' => memo[i - 1][j] || memo[i][j - 1],
                '?' => memo[i - 1][j - 1],
                c => memo[i - 1][j - 1] && c == n[j - 1],
            };
        }
    }
    memo[p.len()][n.len()]
}

/// Walks up from `dir` (inclusive) yielding each ancestor, stopping after
/// `stop` when it is on the path (`FileFinder#traverse_directories_upwards`).
pub(crate) fn ancestors<'a>(dir: &'a Path, stop: Option<&'a Path>) -> Vec<&'a Path> {
    let mut out = Vec::new();
    let mut current = Some(dir);
    while let Some(d) = current {
        out.push(d);
        if Some(d) == stop {
            break;
        }
        current = d.parent();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_and_normalizes() {
        assert_eq!(expand(Path::new("../a/./b.yml"), Path::new("/x/y")), Path::new("/x/a/b.yml"));
        assert_eq!(expand(Path::new("/abs/c.yml"), Path::new("/x")), Path::new("/abs/c.yml"));
    }

    #[test]
    fn relative_uses_parent_hops() {
        assert_eq!(relative(Path::new("/a/b/c.rb"), Path::new("/a")), Path::new("b/c.rb"));
        assert_eq!(relative(Path::new("/a/c.rb"), Path::new("/a/b")), Path::new("../c.rb"));
        assert_eq!(relative(Path::new("/a"), Path::new("/a")), Path::new("."));
    }

    #[test]
    fn wildcard_component_matching() {
        assert!(wildcard_match(".rubocop_*.yml", ".rubocop_todo.yml"));
        assert!(!wildcard_match(".rubocop_*.yml", ".rubocop.yml"));
        assert!(wildcard_match("*", "anything"));
    }

    #[test]
    fn globs_files() {
        let dir = tempfile::tempdir().unwrap();
        for name in [".rubocop_a.yml", ".rubocop_b.yml", "other.yml"] {
            std::fs::write(dir.path().join(name), "").unwrap();
        }
        let found = glob(&dir.path().join(".rubocop_*.yml"));
        let names: Vec<String> =
            found.iter().map(|p| p.file_name().unwrap().to_string_lossy().into()).collect();
        assert_eq!(names, [".rubocop_a.yml", ".rubocop_b.yml"]);
    }
}
