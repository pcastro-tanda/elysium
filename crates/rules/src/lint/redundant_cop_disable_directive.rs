//! `Lint/RedundantCopDisableDirective`, ported from RuboCop's `lib/rubocop/cop/lint/redundant_cop_disable_directive.rb`
//! plus the `DirectiveComment`/`CommentConfig` machinery it relies on (already ported, minus a cop
//! registry, in `ruby_directives`) and RuboCop's `NameSimilarity` (backed by Ruby's `did_you_mean`
//! gem: `DidYouMean::SpellChecker`/`JaroWinkler`/`Levenshtein`, reimplemented here verbatim).
//!
//! Unlike every other rule, this one has no `enter`/`leave` hooks: it runs once per file, in
//! [`Rule::file_finish`], after every other rule has reported its offenses. It replays each
//! `# rubocop:disable`/`todo`/`-next` directive comment's effect cop-by-cop (mirroring RuboCop's
//! `CommentConfig#analyze`, adapted to also remember which directive opened each disabled range)
//! and flags a directive whose covered cop(s) never triggered a real offense in their covered
//! range as redundant.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use linter::{
    Applicability, Context, Department, Diagnostic, Edit, Fix, FixAvailability, OptionError,
    OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
    REDUNDANT_DISABLE_DIRECTIVE_RULE,
};
use regex::Regex;
use ruby_directives::{CopRef, Directive, DirectiveKind};
use ruby_source::Span;

/// RuboCop's `DirectiveComment::LINT_SYNTAX_COP`: never actually disabled by any directive.
const LINT_SYNTAX_COP: &str = "Lint/Syntax";
/// Sentinel line for `RuleOptions`-driven "this cop is disabled in the config" seed ranges,
/// mirroring upstream's `CommentConfig::CONFIG_DISABLED_LINE_RANGE_MIN`. `0` can never be a real
/// (1-based) source line.
const CONFIG_DISABLED_LINE: u32 = 0;

/// Detects `# rubocop:disable`/`todo`/`-next` directives that never suppressed a real offense.
#[derive(Debug, Clone)]
pub struct RedundantCopDisableDirective {
    /// Every cop name the loaded configuration knows about (RuboCop's whole `config/default.yml`
    /// registry, via `RuleOptions::peer_names`), sorted and deduped. Stands in for
    /// `RuboCop::Cop::Registry.global` -- this crate has no cop registry of its own -- for
    /// resolving bare/department cop references and "did you mean" suggestions.
    known_cops: Vec<String>,
    /// Kept so `file_finish` can read another cop's `Enabled` flag (`expected_final_disable?`).
    options: RuleOptions,
}

/// How a directive's raw cop list covered one concrete, resolved cop name.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Via {
    /// `# rubocop:disable all`.
    All,
    /// A real department name (`# rubocop:disable Style`), carrying that department.
    Department(String),
    /// Named directly, whether bare (`MethodLength`) or qualified (`Metrics/MethodLength`).
    Explicit,
}

/// One directive's effect on one specific cop, replayed by [`analyze_cop`].
struct Occ {
    /// Index into the file's directive list; `None` for the synthetic config-disabled seed.
    directive_idx: Option<usize>,
    via: Via,
    line: u32,
    kind: DirectiveKind,
    inline: bool,
}

/// One finished disabled range for one cop, mirroring one entry of RuboCop's
/// `CommentConfig#cop_disabled_line_ranges[cop]`.
struct CopRange {
    start: u32,
    /// `u32::MAX` when the range is still open at end of file.
    end: u32,
    /// The directive that opened this range; `None` only for the sentinel config-disabled seed.
    opener: Option<usize>,
    via: Via,
}

impl Rule for RedundantCopDisableDirective {
    const META: RuleMeta = RuleMeta {
        name: "Lint/RedundantCopDisableDirective",
        department: Department::Lint,
        summary: "Detects instances of rubocop:disable comments that can be removed.",
        explanation: "Detects instances of `rubocop:disable` (and `rubocop:todo`) comments that \
                      can be removed without causing any offenses to be reported. It waits until \
                      every other rule has run and flags any disable directive whose covered \
                      cop(s), department(s), or `all` never suppressed a real offense in the \
                      range it covers.\n\n\
                      ```ruby\n\
                      # bad\n\
                      # rubocop:disable Layout/LineLength\n\
                      x += 1\n\
                      # rubocop:enable Layout/LineLength\n\n\
                      # good\n\
                      x += 1\n\
                      ```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[],
        blind_spots: "`each_already_disabled` (RuboCop's \"this cop was already disabled by an \
                      earlier, still-open directive\" duplicate check, which flags a re-disable \
                      as redundant unconditionally, even over a real offense in its range) is \
                      replicated for the common case a directive names both a department and one \
                      of its own members (`# rubocop:disable Metrics, Metrics/ClassLength`), by \
                      preserving that member's duplicate occurrence instead of collapsing it (see \
                      `expand_directive`); a duplicate re-disable of the exact same cop split \
                      across two *separate* directives with no `enable` between them still falls \
                      back to the ordinary per-range offense check, under-reporting (false \
                      negative, never false positive) the rarer case where the second directive's \
                      range has a genuine offense of its own. `DirectiveComment#malformed?`/`push`/\
                      `pop` directives are out of `ruby_directives`' scope (see its module docs) \
                      and are not handled here either. A directive's trailing free text after the \
                      cop list (`# rubocop:disable Foo -- reason`) is dropped along with the rest \
                      of the comment on a whole-comment removal instead of being preserved as `# \
                      -- reason`. Ambiguous bare cop names (matching more than one department) are \
                      left unresolved (a false negative) instead of raising, since there is no \
                      way to surface RuboCop's `AmbiguousCopName` configuration error here.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let mut known_cops: Vec<String> = options.peer_names().map(str::to_string).collect();
        known_cops.sort();
        known_cops.dedup();
        Ok(Self { known_cops, options: options.clone() })
    }

    fn file_finish(&mut self, ctx: &mut Context<'_>, reported: &[Diagnostic]) {
        self.check(ctx, reported);
    }
}

impl RedundantCopDisableDirective {
    fn check(&self, ctx: &mut Context<'_>, reported: &[Diagnostic]) {
        let directives: Vec<Directive> = ctx.directives().directives().to_vec();
        if directives.is_empty() {
            return;
        }
        let depts = departments(&self.known_cops);
        let expansions: Vec<Vec<(String, Via)>> =
            directives.iter().map(|d| expand_directive(d, &self.known_cops, &depts)).collect();

        let mut touched: BTreeSet<&str> = BTreeSet::new();
        for expansion in &expansions {
            for (key, _) in expansion {
                touched.insert(key.as_str());
            }
        }

        // Directive index -> the set of redundant descriptor keys ("all", "DEPARTMENT<Name>", or
        // a plain resolved cop name) it accumulated, mirroring RuboCop's `redundant_cops` hash of
        // `Set`s keyed by comment.
        let mut redundant: BTreeMap<usize, BTreeSet<String>> = BTreeMap::new();

        for cop in touched {
            let config_disabled = self.cop_disabled_in_config(cop);
            let mut occs: Vec<Occ> = Vec::new();
            if config_disabled {
                occs.push(Occ {
                    directive_idx: None,
                    via: Via::Explicit,
                    line: CONFIG_DISABLED_LINE,
                    kind: DirectiveKind::Disable,
                    inline: false,
                });
            }
            for (idx, expansion) in expansions.iter().enumerate() {
                for (key, via) in expansion {
                    if key != cop {
                        continue;
                    }
                    occs.push(Occ {
                        directive_idx: Some(idx),
                        via: via.clone(),
                        line: directives[idx].line,
                        kind: directives[idx].kind,
                        inline: directives[idx].inline,
                    });
                }
            }
            if occs.is_empty() {
                continue;
            }

            let ranges = analyze_cop(&occs);
            for (range_idx, range) in ranges.iter().enumerate() {
                if range.start == CONFIG_DISABLED_LINE {
                    continue;
                }
                let Some(opener) = range.opener else { continue };

                // `each_already_disabled`: this range's cop was already disabled by an
                // immediately preceding range for the same cop (no `enable` in between, so the
                // previous range ends exactly where this one begins) that wasn't opened by a
                // `disable all` -- redundant unconditionally, even over a real offense or
                // `expected_final_disable?`. A `Department`- or `All`-via range is exempted:
                // their unconditional verdicts (`find_redundant_department`/`find_redundant_all`)
                // are exactly the offense checks below, so there is nothing to short-circuit.
                let already_disabled = range_idx > 0 && {
                    let previous = &ranges[range_idx - 1];
                    previous.end == range.start && previous.via != Via::All
                };
                let unconditional =
                    already_disabled && !matches!(range.via, Via::Department(_) | Via::All);
                if !already_disabled && range.end == u32::MAX && config_disabled {
                    // `expected_final_disable?`: only consulted by the ordinary per-range check,
                    // never by the duplicate-adjacency check above.
                    continue;
                }
                if !unconditional
                    && has_real_offense(ctx, reported, cop, &range.via, range.start, range.end)
                {
                    continue;
                }
                if range.via == Via::All {
                    // `find_redundant_all`: an immediately-following disabled range for this same
                    // cop means a later, more specific directive took over; don't double-flag.
                    if let Some(next) = ranges.get(range_idx + 1) {
                        if next.start == range.end {
                            continue;
                        }
                    }
                }
                let key = match &range.via {
                    Via::All => "all".to_string(),
                    Via::Department(dept) => format!("DEPARTMENT{dept}"),
                    Via::Explicit => cop.to_string(),
                };
                redundant.entry(opener).or_default().insert(key);
            }
        }

        for (idx, keys) in redundant {
            self.report_directive(ctx, &directives[idx], &keys);
        }
    }

    /// RuboCop's `expected_final_disable?`'s config half: is `cop` disabled by the loaded
    /// configuration (as opposed to only by a directive)?
    fn cop_disabled_in_config(&self, cop: &str) -> bool {
        matches!(self.options.peer(cop, "Enabled"), Some(OptionValue::Bool(false)))
    }

    /// Builds and reports the offense(s) for one directive comment, given the redundant
    /// descriptor keys accumulated for it. Mirrors `add_offenses`/`add_offense_for_entire_comment`
    /// /`add_offense_for_some_cops`.
    fn report_directive(
        &self,
        ctx: &mut Context<'_>,
        directive: &Directive,
        keys: &BTreeSet<String>,
    ) {
        let all_disabled = keys.contains("all");
        if all_disabled || directive.cops.len() == keys.len() {
            let mut sorted: Vec<&String> = keys.iter().collect();
            sorted.sort();
            let message = format!(
                "Unnecessary disabling of {}.",
                sorted
                    .iter()
                    .map(|key| describe_key(key, &self.known_cops))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let comment_span = enclosing_comment_span(ctx, directive.span);
            let removal = comment_removal_span(ctx, directive.span, comment_span, directive.inline);
            ctx.report_with_fix(
                &<Self as Rule>::META,
                directive.span,
                message,
                Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(removal)] },
            );
            return;
        }

        let mut spans: Vec<(String, Span)> = keys
            .iter()
            .filter_map(|key| locate_span(ctx, directive.span, key).map(|span| (key.clone(), span)))
            .collect();
        spans.sort_by_key(|(_, span)| span.start);
        let ends_line = spans.last().is_some_and(|(_, span)| ends_its_line(ctx, span.end));
        for i in 0..spans.len() {
            let (key, span) = spans[i].clone();
            let message =
                format!("Unnecessary disabling of {}.", describe_key(&key, &self.known_cops));
            let is_trailing = ends_line && trailing_range(ctx, &spans, i);
            let removal = directive_range_in_list(ctx, span, is_trailing);
            ctx.report_with_fix(
                &<Self as Rule>::META,
                span,
                message,
                Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(removal)] },
            );
        }
    }
}

/// Every real department name our known-cop registry can produce, e.g. `"Style"`, `"Metrics"`.
fn departments(known: &[String]) -> BTreeSet<&str> {
    known.iter().filter_map(|cop| cop.split('/').next()).collect()
}

/// Expands one directive's raw `cops` list into concrete, resolved cop names, mirroring
/// `CommentConfig#analyze`'s `directive.cop_names.each { |cop_name| qualified_cop_name(cop_name) }`
/// (for a plain directive) or `cops == 'all'` short-circuit (for `disable all`). Preserves
/// duplicates and token order (rather than deduping into a map) so a directive naming both a
/// department and one of its own members (`# rubocop:disable Metrics, Metrics/ClassLength`)
/// produces *two* occurrences for that member, mirroring `CommentConfig#analyze`'s
/// `directive.cop_names.each` actually calling `analyze_cop` twice for it (once via the
/// department's expansion, once via the literal mention) -- which is what lets
/// `each_already_disabled` flag only the explicit mention as a redundant duplicate even when a
/// real offense elsewhere in the department justifies the department disable as a whole.
fn expand_directive(
    directive: &Directive,
    known: &[String],
    depts: &BTreeSet<&str>,
) -> Vec<(String, Via)> {
    if directive.cops.iter().any(|cop_ref| matches!(cop_ref, CopRef::All)) {
        return known
            .iter()
            .filter(|cop| {
                cop.as_str() != LINT_SYNTAX_COP && cop.as_str() != REDUNDANT_DISABLE_DIRECTIVE_RULE
            })
            .map(|cop| (cop.clone(), Via::All))
            .collect();
    }

    // Cops named directly (bare or qualified) anywhere in this directive, resolved to concrete
    // names: mirrors `DirectiveComment#overridden_by_department?`'s `raw_cop_names.include?(cop)`
    // check, i.e. which department members this same directive *also* names explicitly.
    let explicit: BTreeSet<String> = directive
        .cops
        .iter()
        .filter_map(|cop_ref| match cop_ref {
            CopRef::Department(name) if !depts.contains(name.as_str()) => {
                Some(qualify_bare(name, known))
            }
            CopRef::Cop(name) => Some(qualify_qualified(name, known)),
            CopRef::Department(_) | CopRef::All => None,
        })
        .collect();

    let mut out: Vec<(String, Via)> = Vec::new();
    for cop_ref in &directive.cops {
        match cop_ref {
            CopRef::All => unreachable!("handled above"),
            CopRef::Department(name) if depts.contains(name.as_str()) => {
                for cop in known.iter().filter(|cop| cop.split('/').next() == Some(name.as_str())) {
                    if name == "Lint"
                        && (cop.as_str() == LINT_SYNTAX_COP
                            || cop.as_str() == REDUNDANT_DISABLE_DIRECTIVE_RULE)
                    {
                        continue;
                    }
                    let via = if explicit.contains(cop) {
                        Via::Explicit
                    } else {
                        Via::Department(name.clone())
                    };
                    out.push((cop.clone(), via));
                }
            }
            // Not a real department: `ruby_directives` has no registry, so a bare cop name like
            // `MethodLength` was classified as `Department` too (see its module docs).
            CopRef::Department(name) => out.push((qualify_bare(name, known), Via::Explicit)),
            CopRef::Cop(name) => out.push((qualify_qualified(name, known), Via::Explicit)),
        }
    }
    out
}

/// RuboCop's `Badge.parse`'s per-segment `camel_case`: `/^[a-z]|_[a-z]/` replaced with the
/// matched letter, uppercased (dropping the underscore, if any).
fn camel_case(part: &str) -> String {
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z]|_[a-z]").expect("valid"));
    if part == "rspec" {
        return "RSpec".to_string();
    }
    RE.replace_all(part, |caps: &regex::Captures<'_>| {
        caps[0].chars().last().map(|c| c.to_ascii_uppercase().to_string()).unwrap_or_default()
    })
    .into_owned()
}

/// Resolves a bare (department-less) cop reference to a real cop name, mirroring
/// `Registry#qualified_cop_name` for an unqualified `Badge`: exactly one registered cop whose
/// name ends in `/<CamelCasedName>` wins; zero or more than one leaves the reference unresolved
/// (upstream raises `AmbiguousCopName` for the latter; false negatives are preferred here).
fn qualify_bare(name: &str, known: &[String]) -> String {
    let camelled = camel_case(name);
    let mut matches = known.iter().filter(|cop| cop.rsplit('/').next() == Some(camelled.as_str()));
    match (matches.next(), matches.next()) {
        (Some(only), None) => only.clone(),
        _ => name.to_string(),
    }
}

/// Resolves an already-qualified (`Department/Cop`) reference, mirroring
/// `Registry#qualified_cop_name` for a qualified `Badge`: if the (`camel_case`d) name is
/// registered as-is, the *original* text is kept (a case/spelling mismatch that still
/// `camel_case`s to a real badge, e.g. `Lint/selfAssignment`, is intentionally not normalized,
/// matching upstream); otherwise, try correcting only the department, keeping the cop-name part
/// fixed (`qualify_badge`); otherwise leave it unresolved.
fn qualify_qualified(name: &str, known: &[String]) -> String {
    let camelled = name.split('/').map(camel_case).collect::<Vec<_>>().join("/");
    if known.iter().any(|cop| cop.as_str() == camelled) {
        return name.to_string();
    }
    let cop_part = camel_case(name.rsplit('/').next().unwrap_or(name));
    let mut matches = known.iter().filter(|cop| cop.rsplit('/').next() == Some(cop_part.as_str()));
    match (matches.next(), matches.next()) {
        (Some(only), None) => only.clone(),
        _ => name.to_string(),
    }
}

/// RuboCop's `CommentConfig#analyze`'s per-cop state machine (`analyze_single_line`/
/// `analyze_disabled`/`analyze_rest`), replayed over one cop's occurrences and additionally
/// remembering which occurrence opened each finished range.
fn analyze_cop(occs: &[Occ]) -> Vec<CopRange> {
    let mut ranges = Vec::new();
    let mut open: Option<(u32, Option<usize>, Via)> = None;
    for occ in occs {
        if occ.kind.is_next() {
            let target = occ.line + 1;
            if occ.kind.disables() {
                ranges.push(CopRange {
                    start: target,
                    end: target,
                    opener: occ.directive_idx,
                    via: occ.via.clone(),
                });
            } else if let Some((start, opener, via)) = open.take() {
                if target > start {
                    ranges.push(CopRange { start, end: target - 1, opener, via });
                }
                open = Some((target + 1, None, Via::Explicit));
            }
        } else if occ.inline {
            if occ.kind.disables() {
                ranges.push(CopRange {
                    start: occ.line,
                    end: occ.line,
                    opener: occ.directive_idx,
                    via: occ.via.clone(),
                });
            }
        } else if occ.kind.disables() {
            if let Some((start, opener, via)) = open.take() {
                ranges.push(CopRange { start, end: occ.line, opener, via });
            }
            open = Some((occ.line, occ.directive_idx, occ.via.clone()));
        } else if let Some((start, opener, via)) = open.take() {
            ranges.push(CopRange { start, end: occ.line, opener, via });
        }
    }
    if let Some((start, opener, via)) = open {
        ranges.push(CopRange { start, end: u32::MAX, opener, via });
    }
    ranges
}

/// Whether a real diagnostic from another rule already justifies this disabled range, mirroring
/// `range_with_offense?`: an `all`-covered range checks every reported offense regardless of cop;
/// a department-covered range checks offenses in that department; an explicitly-named cop checks
/// only its own offenses.
fn has_real_offense(
    ctx: &Context<'_>,
    reported: &[Diagnostic],
    cop: &str,
    via: &Via,
    start: u32,
    end: u32,
) -> bool {
    let line_in_range = |span: Span| {
        let line = ctx.line_col(span.start).line;
        line >= start && line <= end
    };
    match via {
        Via::All => reported.iter().any(|offense| line_in_range(offense.span)),
        Via::Department(dept) => reported.iter().any(|offense| {
            offense.rule.strip_prefix(dept.as_str()).is_some_and(|rest| rest.starts_with('/'))
                && line_in_range(offense.span)
        }),
        Via::Explicit => {
            reported.iter().any(|offense| offense.rule == cop && line_in_range(offense.span))
        }
    }
}

/// Renders one redundant descriptor key for the message, mirroring `describe`.
fn describe_key(key: &str, known: &[String]) -> String {
    if key == "all" {
        return "all cops".to_string();
    }
    if let Some(dept) = key.strip_prefix("DEPARTMENT") {
        return format!("`{dept}` department");
    }
    if known.binary_search_by(|probe| probe.as_str().cmp(key)).is_ok() {
        format!("`{key}`")
    } else if let Some(similar) = find_similar_name(key, known) {
        format!("`{key}` (did you mean `{similar}`?)")
    } else {
        format!("`{key}` (unknown cop)")
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

/// Finds the byte span of `key` (or, failing that, just its cop-name suffix) inside the
/// directive's own text, mirroring `cop_range`/`matching_range`.
fn locate_span(ctx: &Context<'_>, directive_span: Span, key: &str) -> Option<Span> {
    let needle = key.strip_prefix("DEPARTMENT").unwrap_or(key);
    let text = ctx.text(directive_span);
    find_substring(text, needle)
        .or_else(|| find_substring(text, needle.rsplit('/').next().unwrap_or(needle)))
        .map(|(start, len)| {
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

/// Extends `pos` rightward over whitespace bytes; stops at (and never consumes) a `\n` unless
/// `newlines` is set, in which case it continues straight through it.
fn swallow_right(ctx: &Context<'_>, mut pos: u32, newlines: bool) -> u32 {
    let bytes = ctx.source().bytes();
    while (pos as usize) < bytes.len() {
        let byte = bytes[pos as usize];
        if byte == b'\n' && !newlines {
            break;
        }
        if !byte.is_ascii_whitespace() {
            break;
        }
        pos += 1;
    }
    pos
}

/// Mirror of [`swallow_right`], extending leftward.
fn swallow_left(ctx: &Context<'_>, mut pos: u32, newlines: bool) -> u32 {
    let bytes = ctx.source().bytes();
    while pos > 0 {
        let byte = bytes[(pos - 1) as usize];
        if byte == b'\n' && !newlines {
            break;
        }
        if !byte.is_ascii_whitespace() {
            break;
        }
        pos -= 1;
    }
    pos
}

/// The byte range to delete for a whole-comment removal, mirroring
/// `comment_range_with_surrounding_space`. Swallows around the recognized *directive* text
/// (`directive_span`), not the enclosing comment's full span: a comment can carry free text
/// before the marker (`# not very long comment # rubocop:disable Foo`, an inline directive
/// within a single bigger comment) that must survive the removal.
fn comment_removal_span(
    ctx: &Context<'_>,
    directive_span: Span,
    comment_span: Span,
    inline: bool,
) -> Span {
    let line = ctx.line_col(comment_span.start).line;
    let prev_line_blank = line > 1 && ctx.line_text(line - 1).iter().all(u8::is_ascii_whitespace);
    let starts_at_comment_start = directive_span.start == comment_span.start;
    if prev_line_blank && !inline && starts_at_comment_start {
        let end = swallow_right(ctx, directive_span.end, true);
        Span::new(directive_span.start, end)
    } else {
        let at_file_start = directive_span.start == 0;
        let start = swallow_left(ctx, directive_span.start, true);
        let end = swallow_right(ctx, directive_span.end, at_file_start);
        Span::new(start, end)
    }
}

/// The byte range to delete for one cop's entry inside a multi-cop directive list, mirroring
/// `directive_range_in_list`: a trailing entry (nothing but commas ahead of it) eats its leading
/// `, `; any other entry eats its trailing `, `; either way, leftover trailing spaces (not
/// newlines) are swallowed last.
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
    end = swallow_right(ctx, end, false);
    Span::new(start, end)
}

// --- `RuboCop::NameSimilarity`, backed by Ruby's `did_you_mean` gem -------------------------

/// Ports `DidYouMean::SpellChecker#correct` (Ruby's stdlib `did_you_mean` gem), which
/// `RuboCop::NameSimilarity.find_similar_name` (and thus this cop's "did you mean" messages)
/// delegates to.
fn find_similar_name(target: &str, names: &[String]) -> Option<String> {
    let normalized_input = normalize(target);
    let threshold = if normalized_input.chars().count() > 3 { 0.834 } else { 0.77 };

    let mut words: Vec<&String> = names
        .iter()
        .filter(|word| word.as_str() != target)
        .filter(|word| jaro_winkler_distance(&normalize(word), &normalized_input) >= threshold)
        .collect();
    words.sort_by(|a, b| {
        jaro_winkler_distance(a, &normalized_input)
            .partial_cmp(&jaro_winkler_distance(b, &normalized_input))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    words.reverse();

    let mistype_threshold = normalized_input.chars().count().div_ceil(4);
    let mut corrections: Vec<&String> = words
        .iter()
        .copied()
        .filter(|word| {
            levenshtein_distance(&normalize(word), &normalized_input) <= mistype_threshold
        })
        .collect();

    if corrections.is_empty() {
        corrections = words
            .into_iter()
            .filter(|word| {
                let normalized_word = normalize(word);
                let shorter = normalized_input.chars().count().min(normalized_word.chars().count());
                levenshtein_distance(&normalized_word, &normalized_input) < shorter
            })
            .take(1)
            .collect();
    }

    corrections.into_iter().find(|word| word.as_str() != target).cloned()
}

fn normalize(text: &str) -> String {
    text.to_lowercase().replace('@', "")
}

/// Ports `DidYouMean::Jaro.distance`.
#[allow(clippy::cast_precision_loss)]
fn jaro_distance(a: &str, b: &str) -> f64 {
    let (shorter, longer) = if a.chars().count() > b.chars().count() { (b, a) } else { (a, b) };
    let s1: Vec<char> = shorter.chars().collect();
    let s2: Vec<char> = longer.chars().collect();
    let (len1, len2) = (s1.len(), s2.len());
    if len1 == 0 {
        return 0.0;
    }
    let range = if len2 > 3 { len2 / 2 - 1 } else { 0 };
    let mut flags1 = vec![false; len1];
    let mut flags2 = vec![false; len2];
    let mut matches = 0.0_f64;
    for i in 0..len1 {
        let last = i + range;
        let start = i.saturating_sub(range);
        let mut j = start;
        while j <= last && j < len2 {
            if !flags2[j] && s1[i] == s2[j] {
                flags2[j] = true;
                flags1[i] = true;
                matches += 1.0;
                break;
            }
            j += 1;
        }
    }
    if matches == 0.0 {
        return 0.0;
    }
    let mut transpositions = 0.0_f64;
    let mut k = 0usize;
    for (i, &matched) in flags1.iter().enumerate() {
        if !matched {
            continue;
        }
        let mut j = k;
        let mut index;
        loop {
            index = j;
            if flags2[j] {
                k = j + 1;
                break;
            }
            j += 1;
        }
        if s1[i] != s2[index] {
            transpositions += 1.0;
        }
    }
    transpositions = (transpositions / 2.0).floor();
    (matches / len1 as f64 + matches / len2 as f64 + (matches - transpositions) / matches) / 3.0
}

/// Ports `DidYouMean::JaroWinkler.distance`.
#[allow(clippy::cast_precision_loss)]
fn jaro_winkler_distance(a: &str, b: &str) -> f64 {
    let jaro = jaro_distance(a, b);
    if jaro <= 0.7 {
        return jaro;
    }
    let bc: Vec<char> = b.chars().collect();
    let mut prefix = 0usize;
    for (i, ch) in a.chars().enumerate() {
        if prefix >= 4 || bc.get(i) != Some(&ch) {
            break;
        }
        prefix += 1;
    }
    jaro + (prefix as f64 * 0.1 * (1.0 - jaro))
}

/// Ports `DidYouMean::Levenshtein.distance` (standard edit distance; the result is identical to
/// the upstream implementation for every input, only the derivation differs).
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let ca: Vec<char> = a.chars().collect();
    let cb: Vec<char> = b.chars().collect();
    let (n, m) = (ca.len(), cb.len());
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut row: Vec<usize> = (0..=m).collect();
    for i in 1..=n {
        let mut prev_diag = row[0];
        row[0] = i;
        for j in 1..=m {
            let temp = row[j];
            let cost = usize::from(ca[i - 1] != cb[j - 1]);
            row[j] = (row[j] + 1).min(row[j - 1] + 1).min(prev_diag + cost);
            prev_diag = temp;
        }
    }
    row[m]
}
