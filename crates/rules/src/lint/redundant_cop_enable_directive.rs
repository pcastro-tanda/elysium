//! `Lint/RedundantCopEnableDirective`, ported from RuboCop's
//! `lib/rubocop/cop/lint/redundant_cop_enable_directive.rb`, backed by `ruby_directives`'
//! `Directive`/`CopRef` primitives (the pending-disable/extra-enable analysis itself, RuboCop's
//! `CommentConfig#extra_enabled_comments`, is replayed here rather than reused from
//! `Directives::redundant_enables`: that helper collapses a department reference down to one
//! written descriptor, which loses exactly the distinction this cop's autocorrect needs --
//! whether *every* cop in the department was actually redundant (`DirectiveComment#match?`,
//! deciding whether to remove the whole directive or just this one entry from its list) versus
//! only some of them (one department member having been legitimately, individually re-enabled).

use std::sync::LazyLock;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_directives::{CopRef, Directive};
use ruby_source::Span;

/// Detects `# rubocop:enable` directives that never re-enabled a pending disable, mirroring
/// RuboCop's `CommentConfig#extra_enabled_comments`.
#[derive(Debug, Clone)]
pub struct RedundantCopEnableDirective {
    /// Every cop name the loaded configuration knows about (RuboCop's whole `config/default.yml`
    /// registry, via `RuleOptions::peer_names`). Stands in for `RuboCop::Cop::Registry.global` --
    /// this crate has no cop registry of its own -- for expanding `all` and department
    /// references.
    known_cops: Vec<String>,
    /// Kept so `file_end` can read another cop's `Enabled` flag (`registry.disabled(config)`).
    options: RuleOptions,
}

/// One directive whose enabled cop reference(s) were (at least partly) redundant.
struct Redundant {
    span: Span,
    /// True when *every* cop reference in the directive was fully redundant (RuboCop's
    /// `DirectiveComment#match?`): the whole directive comment gets removed. False when at least
    /// one reference was a department only partly covered by redundant members (one of its cops
    /// having been legitimately, individually re-enabled): only this reference's entry in the
    /// directive's cop list gets removed.
    whole: bool,
    /// The redundant reference(s), spelled as written (`all`, a department name, or a cop name),
    /// deduplicated, in written order.
    names: Vec<String>,
}

impl Rule for RedundantCopEnableDirective {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RedundantCopEnableDirective",
        department: Department::Lint,
        summary: "Checks for rubocop:enable comments that can be removed.",
        explanation: "Detects instances of `rubocop:enable` comments that can be removed.\n\n\
                      When comment enables all cops at once `rubocop:enable all` that cop checks \
                      whether any cop was actually enabled.\n\n\
                      ```ruby\n\
                      # bad\n\
                      foo = 1\n\
                      # rubocop:enable Layout/LineLength\n\n\
                      # good\n\
                      foo = 1\n\n\
                      # bad\n\
                      # rubocop:disable Style/StringLiterals\n\
                      foo = \"1\"\n\
                      # rubocop:enable Style/StringLiterals\n\
                      baz\n\
                      # rubocop:enable all\n\n\
                      # good\n\
                      # rubocop:disable Style/StringLiterals\n\
                      foo = \"1\"\n\
                      # rubocop:enable all\n\
                      baz\n\
                      ```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "`-next` directives, out of `ruby_directives`' scope, never count as a \
                      disable or a redundant enable here.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let mut known_cops: Vec<String> = options.peer_names().map(str::to_string).collect();
        known_cops.sort();
        known_cops.dedup();
        Ok(Self { known_cops, options: options.clone() })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let directives: Vec<Directive> = ctx.directives().directives().to_vec();
        if directives.is_empty() {
            return;
        }
        // `registry.disabled_names(config)` seeds one pending disable per config-disabled cop.
        // Under `--only` the run's registry is exactly the listed cops, all enabled, so nothing
        // is seeded. Otherwise only cops this file's directives mention can matter, so the
        // search is limited to those.
        let only_run = self.options.only_run();
        let disabled_by_config: Vec<String> = self
            .known_cops
            .iter()
            .filter(|cop| {
                !only_run
                    && matches!(self.options.peer(cop, "Enabled"), Some(OptionValue::Bool(false)))
                    && directives
                        .iter()
                        .any(|d| d.cops.iter().any(|c| matches!(c, CopRef::Cop(n) if n == *cop)))
            })
            .cloned()
            .collect();
        for redundant in analyze(&directives, &self.known_cops, &disabled_by_config) {
            Self::register_offense(ctx, &redundant);
        }
    }
}

impl RedundantCopEnableDirective {
    /// Reports one offense per redundant name in `redundant`, mirroring `register_offense`.
    fn register_offense(ctx: &mut Context<'_>, redundant: &Redundant) {
        let comment_span = enclosing_comment_span(ctx, redundant.span);

        let mut spans: Vec<(String, Span)> = redundant
            .names
            .iter()
            .filter_map(|name| {
                locate_span(ctx, redundant.span, name).map(|span| (name.clone(), span))
            })
            .collect();
        spans.sort_by_key(|(_, span)| span.start);
        let ends_line = spans.last().is_some_and(|(_, span)| ends_its_line(ctx, span.end));

        for (i, (name, span)) in spans.iter().enumerate() {
            let message = format!("Unnecessary enabling of {}.", all_or_name(name));
            let removal = if redundant.whole {
                Span::new(redundant.span.start, swallow_directive_end(ctx, redundant.span.end))
            } else if has_adjacent_comma(ctx, *span) {
                let is_trailing = ends_line && trailing_range(ctx, &spans, i);
                directive_range_in_list(ctx, *span, is_trailing)
            } else {
                // `range_to_remove`'s fallback: this reference is alone in the directive's cop
                // list (no comma to eat on either side) yet the directive as a whole isn't fully
                // redundant (a sibling department member was legitimately re-enabled elsewhere);
                // remove the whole comment's own span, without swallowing surrounding
                // whitespace/newlines.
                comment_span
            };
            ctx.report_with_fix(
                &<Self as Rule>::META,
                *span,
                message,
                Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(removal)] },
            );
        }
    }
}

/// Replays RuboCop's `CommentConfig#extra_enabled_comments` (a pending-disable-count state
/// machine over every own-line, non-`-next` directive with a non-empty cop list) far enough to
/// recover, for every redundantly-enabled directive, both which references it redundantly
/// enables and whether *every* cop each department reference stands for was actually redundant.
fn analyze(
    directives: &[Directive],
    universe: &[String],
    disabled_by_config: &[String],
) -> Vec<Redundant> {
    use std::collections::BTreeMap;

    let mut pending: BTreeMap<String, u32> = BTreeMap::new();
    for name in disabled_by_config {
        *pending.entry(name.clone()).or_default() += 1;
    }

    let mut out = Vec::new();
    let relevant =
        directives.iter().filter(|d| !d.inline && !d.kind.is_next() && !d.cops.is_empty());
    for directive in relevant {
        if directive.kind.disables() {
            for cop in &directive.cops {
                for member in expand(universe, cop) {
                    *pending.entry(member).or_default() += 1;
                }
            }
            continue;
        }

        // `handle_enable_all`: `enable all` is redundant only when it enables nothing (no cop had
        // a pending disable); otherwise it clears one pending disable from every cop that has
        // one, whether or not that leaves the directive itself flagged.
        if directive.cops.as_slice() == [CopRef::All] {
            let mut enabled = 0_u32;
            for count in pending.values_mut() {
                if *count > 0 {
                    *count -= 1;
                    enabled += 1;
                }
            }
            if enabled == 0 {
                out.push(Redundant {
                    span: directive.span,
                    whole: true,
                    names: vec!["all".to_string()],
                });
            }
            continue;
        }

        let mut names: Vec<String> = Vec::new();
        let mut whole = true;
        let mut any_extra = false;
        for cop in &directive.cops {
            let members = expand(universe, cop);
            let mut extra_count = 0_usize;
            for member in &members {
                if let Some(count) = pending.get_mut(member).filter(|count| **count > 0) {
                    *count -= 1;
                } else {
                    extra_count += 1;
                }
            }
            if extra_count == 0 {
                // Fully legitimate re-enable: every cop this reference stands for had a pending
                // disable. A directive mixing this with an extra reference elsewhere is not
                // fully redundant (RuboCop's `match?` compares the *whole* directive's parsed cop
                // set against its extra set).
                whole = false;
                continue;
            }
            any_extra = true;
            if extra_count != members.len() {
                whole = false;
            }
            let written = match cop {
                CopRef::All => "all".to_string(),
                CopRef::Department(name) | CopRef::Cop(name) => name.clone(),
            };
            if !names.contains(&written) {
                names.push(written);
            }
        }
        if any_extra {
            out.push(Redundant { span: directive.span, whole, names });
        }
    }
    out
}

/// Every cop name `cop` stands for, against the known cop universe -- mirrors
/// `Directives::redundant_enables`'s private `expand`. A reference the universe does not know
/// (an unknown or misspelled cop, or a universe that was never supplied) stands for itself.
fn expand(universe: &[String], cop: &CopRef) -> Vec<String> {
    if matches!(cop, CopRef::All) {
        return universe.to_vec();
    }
    let matched: Vec<String> = universe.iter().filter(|name| cop.covers(name)).cloned().collect();
    if matched.is_empty() {
        match cop {
            CopRef::Department(name) | CopRef::Cop(name) => vec![name.clone()],
            CopRef::All => Vec::new(),
        }
    } else {
        matched
    }
}

/// RuboCop's `all_or_name`.
fn all_or_name(name: &str) -> &str {
    if name == "all" {
        "all cops"
    } else {
        name
    }
}

/// The full comment containing `directive_span` (the comment's own span may extend past the
/// recognized directive text, e.g. trailing `-- reason` free text).
fn enclosing_comment_span(ctx: &Context<'_>, directive_span: Span) -> Span {
    ctx.comments()
        .iter()
        .find(|comment| {
            comment.span.start <= directive_span.start && directive_span.end <= comment.span.end
        })
        .map_or(directive_span, |comment| comment.span)
}

/// Finds the byte span of `name` inside the directive's own text, mirroring
/// `cop_name_indention`/`range_of_offense`.
fn locate_span(ctx: &Context<'_>, directive_span: Span, name: &str) -> Option<Span> {
    let text = ctx.text(directive_span);
    find_substring(text, name).map(|(start, len)| {
        let start = u32::try_from(start).unwrap_or(u32::MAX);
        let len = u32::try_from(len).unwrap_or(u32::MAX);
        Span::new(directive_span.start + start, directive_span.start + start + len)
    })
}

fn find_substring(haystack: &[u8], needle: &str) -> Option<(usize, usize)> {
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|pos| (pos, needle.len()))
}

/// True when nothing but whitespace follows `pos` through the end of its line, mirroring
/// `ends_its_line?`.
fn ends_its_line(ctx: &Context<'_>, pos: u32) -> bool {
    let line = ctx.line_col(pos).line;
    let line_end = ctx.line_span(line).end;
    ctx.text(Span::new(pos, line_end)).iter().all(u8::is_ascii_whitespace)
}

/// True when, from `spans[i]` onward, every consecutive pair is separated by nothing but
/// `,`-plus-whitespace, mirroring `trailing_range?`.
fn trailing_range(ctx: &Context<'_>, spans: &[(String, Span)], i: usize) -> bool {
    static SEPARATOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*,\s*$").expect("valid"));
    spans[i..].windows(2).all(|pair| {
        let gap = ctx.text(Span::new(pair[0].1.end, pair[1].1.start));
        std::str::from_utf8(gap).is_ok_and(|text| SEPARATOR.is_match(text))
    })
}

/// True when `span` has a `,` immediately before or after it (ignoring surrounding spaces/tabs):
/// this reference is one entry in a comma-separated cop list, mirroring `range_to_remove`'s two
/// comma checks.
fn has_adjacent_comma(ctx: &Context<'_>, span: Span) -> bool {
    let bytes = ctx.source().bytes();
    let mut before = span.start as usize;
    while before > 0 && bytes[before - 1] == b' ' {
        before -= 1;
    }
    let has_before = before > 0 && bytes[before - 1] == b',';
    let mut after = span.end as usize;
    while after < bytes.len() && bytes[after] == b' ' {
        after += 1;
    }
    let has_after = after < bytes.len() && bytes[after] == b',';
    has_before || has_after
}

/// Extends `pos` rightward over trailing spaces/tabs, then over every immediately following
/// newline, mirroring `RangeHelp#final_pos`'s right-side walk (`whitespace: false`,
/// `newlines: true`, the defaults `range_with_surrounding_space(directive.range, side: :right)`
/// uses).
fn swallow_directive_end(ctx: &Context<'_>, pos: u32) -> u32 {
    let bytes = ctx.source().bytes();
    let mut p = pos as usize;
    while p < bytes.len() && matches!(bytes[p], b' ' | b'\t') {
        p += 1;
    }
    while p < bytes.len() && bytes[p] == b'\n' {
        p += 1;
    }
    u32::try_from(p).unwrap_or(u32::MAX)
}

/// The byte range to delete for one name's entry inside a multi-cop directive list, mirroring
/// `range_with_comma`/`range_to_remove` for the comma-adjacent case: a trailing entry (nothing
/// but commas ahead of it) eats its leading `, `; any other entry eats its trailing `, `; either
/// way, leftover trailing spaces (not newlines) are swallowed last.
fn directive_range_in_list(ctx: &Context<'_>, span: Span, is_trailing: bool) -> Span {
    let bytes = ctx.source().bytes();
    let mut start = span.start;
    let mut end = span.end;
    if is_trailing {
        let mut probe = start;
        while probe > 0 && bytes[(probe - 1) as usize] == b' ' {
            probe -= 1;
        }
        if probe > 0 && bytes[(probe - 1) as usize] == b',' {
            probe -= 1;
            while probe > 0 && bytes[(probe - 1) as usize] == b' ' {
                probe -= 1;
            }
            start = probe;
        }
    } else {
        let mut probe = end;
        while (probe as usize) < bytes.len() && bytes[probe as usize] == b' ' {
            probe += 1;
        }
        if (probe as usize) < bytes.len() && bytes[probe as usize] == b',' {
            probe += 1;
            while (probe as usize) < bytes.len() && bytes[probe as usize] == b' ' {
                probe += 1;
            }
            end = probe;
        }
    }
    let mut probe = end;
    while (probe as usize) < bytes.len() && bytes[probe as usize] == b' ' {
        probe += 1;
    }
    Span::new(start, probe)
}
