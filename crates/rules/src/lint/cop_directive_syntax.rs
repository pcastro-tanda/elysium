//! `Lint/CopDirectiveSyntax`, ported from RuboCop's
//! `lib/rubocop/cop/lint/cop_directive_syntax.rb`, plus the raw-text
//! `RuboCop::DirectiveComment` parsing (`lib/rubocop/directive_comment.rb`) it
//! relies on for `malformed?`/`missing_cop_name?`/`invalid_signed_args?`.
//!
//! # Why this duplicates `ruby_directives`'s regex instead of reusing it
//!
//! `ruby_directives::Directives` already parses `# rubocop:` directive
//! comments, but its module docs are explicit that it drops anything
//! malformed: an invalid mode, a missing cop name, an unsigned `push`/`next`
//! argument, and trailing free text after the cop list are all silently
//! ignored there, because every other cop only cares about a directive's
//! *effect*. This cop's entire job is diagnosing exactly those malformed
//! shapes (plus scanning a comment for more than one directive, which
//! `Directives` never does -- it keeps only the first match per comment).
//! That needs the raw match upstream's `DirectiveComment` exposes -- the
//! mode, the unbranched cops-or-signed text, and the text following the
//! match -- which `ruby_directives` deliberately does not keep. Per the
//! porting kit, that parsing is copied privately into this file instead of
//! reshaping the shared crate around one cop's diagnostics.
//!
//! `ctx.directives().comment_only_line` *is* reused for the one check that
//! genuinely depends on the whole file's line classification
//! (`misplaced_next_directive?`).

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::bytes::Regex;
use ruby_source::Span;
use std::collections::BTreeSet;
use std::sync::LazyLock;

use crate::name_similarity::{find_similar_name, levenshtein};

const COMMON_MSG: &str = "Malformed directive comment detected.";
const MISSING_MODE_NAME_MSG: &str = "The mode name is missing.";
const INVALID_MODE_NAME_MSG: &str = "The mode name must be one of `enable`, `disable`, \
                                      `disable-next`, `enable-next`, `todo`, `todo-next`, \
                                      `next`, `push`, or `pop`.";
const MISSING_COP_NAME_MSG: &str = "The cop name is missing.";
const MULTIPLE_DIRECTIVES_MSG: &str = "Only the first directive on a line takes effect. List \
                                        the cop names in a single directive instead.";
const MALFORMED_COP_NAMES_MSG: &str = "Cop names must be separated by commas. Comment in the \
                                        directive must start with `--`.";
const INVALID_SIGNED_ARGS_MSG: &str = "`push` and `next` arguments must be `+`- or `-`-prefixed \
                                        cop names, and `pop` takes no arguments.";
const NEXT_DIRECTIVE_AT_EOL_MSG: &str = "A `-next` directive must be on its own line, above the \
                                          statement it applies to.";

/// Upstream's `DirectiveComment::AVAILABLE_MODES`.
const AVAILABLE_MODES: &[&str] = &[
    "disable",
    "enable",
    "todo",
    "push",
    "pop",
    "disable-next",
    "todo-next",
    "enable-next",
    "next",
];

// The mode alternation, repeated in every regex below: longest first, so a
// `-next` mode is not matched as a prefix of its bare mode (see
// `ruby_directives`, which explains the same ordering requirement).
const MODES_ALT: &str = "disable-next|enable-next|todo-next|disable|enable|push|todo|next|pop";

/// Upstream's `DirectiveComment::DIRECTIVE_COMMENT_REGEXP`, with both the
/// `cops` and `signed` groups always available (unlike `ruby_directives`,
/// which keeps only the one its directive's mode expects).
static DIRECTIVE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?-u)\#\s*rubocop\s*:\s*(?P<mode>{MODES_ALT})\b(?:\s+(?P<cops>all|(?:[A-Za-z]\w+/)*[A-Za-z]\w+(?:\s*,\s*(?:[A-Za-z]\w+/)*[A-Za-z]\w+)*)|\s+(?P<signed>[+\-](?:[A-Za-z]\w+/)*[A-Za-z]\w+(?:\s+[+\-](?:[A-Za-z]\w+/)*[A-Za-z]\w+)*))?"
    ))
    .expect("static directive regex is valid")
});

/// Upstream's `DirectiveComment::DIRECTIVE_MARKER_REGEXP` (unanchored, so it
/// can be used both for `start_with?` -- checked at position 0 -- and for
/// `sub` -- the first occurrence anywhere).
static MARKER_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u)\#\s*rubocop\s*:\s*").expect("static marker regex is valid")
});

/// Upstream's `DirectiveComment::MALFORMED_DIRECTIVE_WITHOUT_COP_NAME_REGEXP`.
static MISSING_COP_NAME_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?-u)\A\#\s*rubocop\s*:\s*(?:{MODES_ALT})\b\s*\z"))
        .expect("static missing-cop-name regex is valid")
});

/// Upstream's `CopDirectiveSyntax::NEAR_MISS_KEYWORD_REGEXP`.
static NEAR_MISS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?-u)\A\#+\s*(?P<keyword>[A-Za-z][\w-]*)\s*:\s*(?:{MODES_ALT})\b"))
        .expect("static near-miss regex is valid")
});

/// One match of [`DIRECTIVE_REGEX`] against a comment's own text -- upstream's
/// `DirectiveComment`, minus everything that only exists to compute a
/// directive's *effect* (that part is `ruby_directives::Directive`).
#[derive(Debug, Clone)]
struct DirectiveMatch<'t> {
    /// Byte offset of the match's start within the comment text.
    start: usize,
    /// Byte offset of the match's end within the comment text.
    end: usize,
    mode: &'t str,
    /// Text captured by the `cops` group specifically (a plain or `all` cop
    /// list) -- upstream's raw `MatchData[2]`, used unmixed with `signed` by
    /// [`merged_cops`], which only ever folds plain cop lists together.
    cops_group: Option<&'t str>,
    /// Text captured by the `signed` group specifically (`push`/`next`
    /// arguments).
    signed_group: Option<&'t str>,
}

impl<'t> DirectiveMatch<'t> {
    /// Upstream's `DirectiveComment#cops`: whichever of the two groups
    /// matched, preferring `cops`.
    fn cops_text(&self) -> Option<&'t str> {
        self.cops_group.or(self.signed_group)
    }
}

/// `DirectiveComment::MODES_PATTERN`'s union of `disable`/`todo` -- the only
/// pair `merged_mode` folds together when they differ.
fn is_next_mode(mode: &str) -> bool {
    matches!(mode, "disable-next" | "todo-next" | "enable-next" | "next")
}

/// `\A#\s*\z`: upstream drops a match whose `pre_match` is nothing but a
/// second comment marker, so a commented-out directive (`# # rubocop:disable
/// Foo`) is an ordinary comment rather than a directive.
fn is_commented_out(prefix: &[u8]) -> bool {
    matches!(prefix.split_first(), Some((b'#', rest)) if rest.iter().all(u8::is_ascii_whitespace))
}

fn to_match<'t>(caps: &regex::bytes::Captures<'t>) -> DirectiveMatch<'t> {
    let whole = caps.get(0).expect("regex match always has a full match");
    let mode = std::str::from_utf8(caps.name("mode").expect("mode always captured").as_bytes())
        .unwrap_or_default();
    let bytes_to_str =
        |m: regex::bytes::Match<'t>| std::str::from_utf8(m.as_bytes()).unwrap_or_default();
    DirectiveMatch {
        start: whole.start(),
        end: whole.end(),
        mode,
        cops_group: caps.name("cops").map(bytes_to_str),
        signed_group: caps.name("signed").map(bytes_to_str),
    }
}

/// Upstream's `DirectiveComment#initialize`'s `@match_data`: the leftmost
/// match, discarded when it is only reachable through a commented-out prefix.
fn find_first_directive(text: &[u8]) -> Option<DirectiveMatch<'_>> {
    let caps = DIRECTIVE_REGEX.captures(text)?;
    let whole = caps.get(0).expect("regex match always has a full match");
    if is_commented_out(&text[..whole.start()]) {
        return None;
    }
    Some(to_match(&caps))
}

/// Upstream's `comment.text.to_enum(:scan, DIRECTIVE_COMMENT_REGEXP)`: every
/// non-overlapping match, left to right, with no commented-out filtering
/// (that filtering only ever applied to the singular `@match_data`).
fn scan_directives(text: &[u8]) -> Vec<DirectiveMatch<'_>> {
    DIRECTIVE_REGEX.captures_iter(text).map(|caps| to_match(&caps)).collect()
}

/// Upstream's `DirectiveComment#start_with_marker?`.
fn start_with_marker(text: &[u8]) -> bool {
    MARKER_REGEX.find(text).is_some_and(|m| m.start() == 0)
}

/// The comment text after the first marker occurrence -- upstream's
/// `comment.text.sub(DIRECTIVE_MARKER_REGEXP, '')`, without the discarded
/// prefix (empty whenever `start_with_marker` holds, which is the only time
/// this is called).
fn after_marker(text: &[u8]) -> &[u8] {
    MARKER_REGEX.find(text).map_or(text, |m| &text[m.end()..])
}

fn ltrim(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(bytes.len());
    &bytes[start..]
}

/// Ruby's `"...".split(' ', 2).first`: the first whitespace-delimited token,
/// ignoring any leading whitespace; `None` when nothing but whitespace
/// remains.
fn first_token(bytes: &[u8]) -> Option<&str> {
    let trimmed = ltrim(bytes);
    if trimmed.is_empty() {
        return None;
    }
    let end = trimmed.iter().position(u8::is_ascii_whitespace).unwrap_or(trimmed.len());
    std::str::from_utf8(&trimmed[..end]).ok()
}

/// Upstream's `DirectiveComment#missing_cop_name?`.
fn missing_cop_name(text: &[u8], mode: &str) -> bool {
    if mode == "push" || mode == "pop" {
        return false;
    }
    MISSING_COP_NAME_REGEX.is_match(text)
}

/// Upstream's `DirectiveComment#invalid_signed_args?`.
fn invalid_signed_args(mode: &str, cops_text: Option<&str>) -> bool {
    if mode == "pop" {
        return cops_text.is_some_and(|c| !c.is_empty());
    }
    if mode != "push" && mode != "next" {
        return false;
    }
    let Some(cops) = cops_text else { return false };
    cops.split_ascii_whitespace().any(|spec| !(spec.starts_with('+') || spec.starts_with('-')))
}

/// Upstream's `DirectiveComment#reason`, reduced to whether one exists at
/// all (its text is never used in a message).
fn has_reason(text: &[u8]) -> bool {
    let Some(m) = find_first_directive(text) else { return false };
    let tail = ltrim(&text[m.end..]);
    let Some(rest) = tail.strip_prefix(b"--") else { return false };
    !ltrim(rest).is_empty()
}

/// Upstream's `DirectiveComment#malformed?`.
fn malformed(text: &[u8]) -> bool {
    if !start_with_marker(text) {
        return true;
    }
    let Some(m) = find_first_directive(text) else { return true };
    if missing_cop_name(text, m.mode) || invalid_signed_args(m.mode, m.cops_text()) {
        return true;
    }
    let tail = ltrim(&text[m.end..]);
    !(tail.is_empty() || tail.starts_with(b"--"))
}

/// Upstream's `CopDirectiveSyntax#offense_message`.
fn offense_message(text: &[u8]) -> String {
    let additional = match first_token(after_marker(text)) {
        None => MISSING_MODE_NAME_MSG,
        Some(token) if !AVAILABLE_MODES.contains(&token) => INVALID_MODE_NAME_MSG,
        Some(_) => {
            let m = find_first_directive(text);
            let mode = m.as_ref().map_or("", |m| m.mode);
            let cops_text = m.as_ref().and_then(DirectiveMatch::cops_text);
            if missing_cop_name(text, mode) {
                MISSING_COP_NAME_MSG
            } else if invalid_signed_args(mode, cops_text) {
                INVALID_SIGNED_ARGS_MSG
            } else {
                MALFORMED_COP_NAMES_MSG
            }
        }
    };
    format!("{COMMON_MSG} {additional}")
}

/// Upstream's `CopDirectiveSyntax#multiple_directives` (the scan) plus
/// `#filter_reason_matches`.
fn multiple_directives(text: &[u8]) -> Option<Vec<DirectiveMatch<'_>>> {
    let matches = scan_directives(text);
    if matches.len() < 2 {
        return None;
    }
    let filtered = if has_reason(text) {
        let mut kept = vec![matches[0].clone()];
        kept.extend(matches[1..].iter().filter(|m| ltrim(&text[m.end..]).is_empty()).cloned());
        kept
    } else {
        matches
    };
    (filtered.len() > 1).then_some(filtered)
}

/// Upstream's `CopDirectiveSyntax#directives_adjacent?`.
fn directives_adjacent(text: &[u8], matches: &[DirectiveMatch<'_>]) -> bool {
    for i in 0..matches.len() {
        let from = matches[i].end;
        let to = if i + 1 < matches.len() { matches[i + 1].start } else { text.len() };
        if from > to || !ltrim(&text[from..to]).is_empty() {
            return false;
        }
    }
    true
}

/// Upstream's `CopDirectiveSyntax#merged_mode`.
fn merged_mode<'t>(matches: &[DirectiveMatch<'t>]) -> Option<&'t str> {
    let mut modes: Vec<&str> = Vec::new();
    for m in matches {
        if !modes.contains(&m.mode) {
            modes.push(m.mode);
        }
    }
    if modes.len() == 1 {
        return Some(modes[0]);
    }
    if modes.iter().all(|mode| *mode == "disable" || *mode == "todo") {
        return Some(modes[0]);
    }
    None
}

/// Upstream's `CopDirectiveSyntax#merged_cops`. Only the `cops` group is
/// consulted (upstream indexes the raw `MatchData` at the `COPS_PATTERN`
/// capture specifically), so a `push`/`next` match -- which only ever
/// populates `signed` -- contributes nothing.
fn merged_cops(matches: &[DirectiveMatch<'_>]) -> Option<Vec<String>> {
    let mut cops: Vec<String> = Vec::new();
    for m in matches {
        let Some(text) = m.cops_group else { continue };
        for part in text.split(',') {
            let trimmed = part.trim();
            if !trimmed.is_empty() && !cops.iter().any(|c| c == trimmed) {
                cops.push(trimmed.to_string());
            }
        }
    }
    if cops.is_empty() || cops.iter().any(|c| c == "all") {
        None
    } else {
        Some(cops)
    }
}

/// Upstream's `CopDirectiveSyntax#merge_directives`: the autocorrection for
/// `MULTIPLE_DIRECTIVES_MSG`, or `None` when upstream also declines to fix.
fn merge_replacement(text: &[u8], matches: &[DirectiveMatch<'_>]) -> Option<Vec<u8>> {
    if !directives_adjacent(text, matches) {
        return None;
    }
    let mode = merged_mode(matches)?;
    let cops = merged_cops(matches)?;
    Some(format!("# rubocop:{mode} {}", cops.join(", ")).into_bytes())
}

/// Upstream's `CopDirectiveSyntax#near_miss_keyword`.
fn near_miss_keyword(text: &[u8]) -> Option<&str> {
    let caps = NEAR_MISS_REGEX.captures(text)?;
    let keyword =
        std::str::from_utf8(caps.name("keyword").expect("keyword always captured").as_bytes())
            .ok()?;
    if keyword == "rubocop" {
        return None;
    }
    similar_to_rubocop(keyword).then_some(keyword)
}

/// Upstream's `CopDirectiveSyntax#similar_to_rubocop?`.
fn similar_to_rubocop(keyword: &str) -> bool {
    if keyword.eq_ignore_ascii_case("rubocop") {
        return true;
    }
    let keyword: Vec<char> = keyword.to_lowercase().chars().collect();
    let rubocop: Vec<char> = "rubocop".chars().collect();
    levenshtein(&keyword, &rubocop) <= 2
}

/// Upstream's `CopDirectiveSyntax#directive_names`.
fn directive_names(mode: &str, cops_text: &str) -> Vec<String> {
    if mode == "push" || mode == "next" {
        cops_text
            .split_ascii_whitespace()
            .filter_map(|spec| spec.strip_prefix('+').or_else(|| spec.strip_prefix('-')))
            .map(str::to_string)
            .collect()
    } else {
        cops_text.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect()
    }
}

/// Every department name the known-cop registry can produce, e.g. `"Style"`.
fn departments(known: &[String]) -> BTreeSet<&str> {
    known.iter().filter_map(|cop| cop.split('/').next()).collect()
}

/// Upstream's `registry.qualified_cop_name(name, nil, warn: false)` plus
/// `registry.contains_cop_matching?`: a slash-qualified name must match
/// exactly, an unqualified one resolves against any known cop sharing that
/// bare name (RuboCop's built-in registry never has two cops sharing a bare
/// name, so this never hits the ambiguous case upstream would raise on).
fn is_known(name: &str, known: &[String]) -> bool {
    if name.contains('/') {
        known.binary_search_by(|k| k.as_str().cmp(name)).is_ok()
    } else {
        known.iter().any(|k| k.rsplit('/').next() == Some(name))
    }
}

/// Upstream's `CopDirectiveSyntax#unknown_cop_name`.
fn unknown_cop_name(text: &[u8], known: &[String], depts: &BTreeSet<&str>) -> Option<String> {
    let m = find_first_directive(text)?;
    if m.mode == "pop" {
        return None;
    }
    let cops_text = m.cops_text().unwrap_or_default();
    if cops_text == "all" {
        return None;
    }
    directive_names(m.mode, cops_text)
        .into_iter()
        .find(|name| !depts.contains(name.as_str()) && !is_known(name, known))
}

fn unknown_cop_message(name: &str, known: &[String]) -> String {
    let suggestion = find_similar_name(name, known)
        .map(|similar| format!(" (did you mean `{similar}`?)"))
        .unwrap_or_default();
    format!("Unknown cop name `{name}`{suggestion}.")
}

/// Checks that `# rubocop:` directives are strictly formatted.
#[derive(Debug, Clone)]
pub struct CopDirectiveSyntax {
    /// Every cop name the loaded configuration knows about, sorted and
    /// deduped -- see `Lint/RedundantCopDisableDirective`'s `known_cops` for
    /// why this stands in for `RuboCop::Cop::Registry.global` here.
    known_cops: Vec<String>,
}

impl CopDirectiveSyntax {
    fn check_directive(&self, ctx: &mut Context<'_>, span: Span, line: u32, text: &[u8]) {
        if let Some(matches) = multiple_directives(text) {
            report_multiple_directives(ctx, span, text, &matches);
            return;
        }
        if malformed(text) {
            ctx.report(&<Self as Rule>::META, span, offense_message(text));
            return;
        }
        let m = find_first_directive(text).expect("malformed()==false implies a match");
        if is_next_mode(m.mode) && !ctx.directives().comment_only_line(line) {
            ctx.report(&<Self as Rule>::META, span, NEXT_DIRECTIVE_AT_EOL_MSG);
            return;
        }
        if let Some(name) = unknown_cop_name(text, &self.known_cops, &departments(&self.known_cops))
        {
            let message = unknown_cop_message(&name, &self.known_cops);
            ctx.report(&<Self as Rule>::META, span, message);
        }
    }
}

fn report_multiple_directives(
    ctx: &mut Context<'_>,
    span: Span,
    text: &[u8],
    matches: &[DirectiveMatch<'_>],
) {
    let message = format!("{COMMON_MSG} {MULTIPLE_DIRECTIVES_MSG}");
    if let Some(replacement) = merge_replacement(text, matches) {
        ctx.report_with_fix(
            &<CopDirectiveSyntax as Rule>::META,
            span,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(span, replacement)],
            },
        );
    } else {
        ctx.report(&<CopDirectiveSyntax as Rule>::META, span, message);
    }
}

impl Rule for CopDirectiveSyntax {
    const META: RuleMeta = RuleMeta {
        name: "Lint/CopDirectiveSyntax",
        department: Department::Lint,
        summary: "Checks that `# rubocop:` directives are strictly formatted.",
        explanation: "\
Checks that `# rubocop:enable ...` and `# rubocop:disable ...` statements are \
strictly formatted.

A comment can be added to the directive by prefixing it with `--`.

```ruby
# bad
# rubocop:disable Layout/LineLength Style/Encoding

# good
# rubocop:disable Layout/LineLength, Style/Encoding

# bad
# rubocop:disable

# good
# rubocop:disable all

# bad - only the first directive takes effect
# rubocop:disable Layout/LineLength # rubocop:disable Style/Encoding

# good
# rubocop:disable Layout/LineLength, Style/Encoding

# bad
# rubocop:wrongmode Layout/LineLength

# good
# rubocop:disable Layout/LineLength

# bad
# rubocop:disable Layout/LineLength comment

# good
# rubocop:disable Layout/LineLength -- comment

# bad
# rucocop:disable Layout/LineLength

# good
# rubocop:disable Layout/LineLength

# bad
# rubocop:disable Layout/LineLenght

# good
# rubocop:disable Layout/LineLength
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[],
        blind_spots: "\
Resolving an unqualified cop name (e.g. `# rubocop:disable LineLength`) picks \
any known cop sharing that bare name instead of raising on an ambiguous match \
like RuboCop's registry would; the built-in registry never has two cops \
sharing a bare name, so this never actually differs.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let mut known_cops: Vec<String> = options.peer_names().map(str::to_string).collect();
        known_cops.sort();
        known_cops.dedup();
        Ok(Self { known_cops })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let comments = ctx.comments().to_vec();
        for comment in &comments {
            let text = ctx.text(comment.span);
            if start_with_marker(text) {
                self.check_directive(ctx, comment.span, comment.line, text);
            } else if let Some(keyword) = near_miss_keyword(text) {
                let message = format!(
                    "{COMMON_MSG} The directive keyword must be `rubocop`, not `{keyword}`."
                );
                ctx.report(&Self::META, comment.span, message);
            }
        }
    }
}
