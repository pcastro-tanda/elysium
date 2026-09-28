//! `Lint/MissingCopEnableDirective`, ported from RuboCop's
//! `lib/rubocop/cop/lint/missing_cop_enable_directive.rb`.
//!
//! # No cop registry, so ranges are grouped by literal reference, not by real cop name
//!
//! Upstream's `CommentConfig#analyze` first expands every directive's `all`/department
//! references against the live cop registry, then replays the whole directive stream once per
//! *real, concrete* cop name (`CommentConfig::cop_disabled_line_ranges`); a bare `# rubocop:disable
//! Layout` therefore opens an identical range for every real `Layout/*` cop, and a later `#
//! rubocop:enable Layout/Foo` closes only `Layout/Foo`'s copy of it, leaving every other `Layout/*`
//! cop's copy open. This crate has no cop registry ([`ruby_directives`]' own module docs document
//! the same limitation), so ranges are instead grouped by the directive's own literal reference
//! (`"all"`, a bare department word, or a slash-qualified cop path) and replayed once per literal
//! key. This matches upstream exactly for the common case -- a department or `all` is disabled and
//! re-enabled by the same literal spelling, never partially reactivated cop-by-cop -- and only
//! under- or over-reports for the rarer mixed case where a department-wide disable is later
//! narrowed by naming one of its own members. `add_offense`'s own per-range dedup
//! (`current_offense_locations.add?`) is replicated via `reported_spans` in [`check`], since every
//! real cop sharing one department's range would otherwise each independently, but identically,
//! offend on the same comment.
//!
//! The `line_range.min == CommentConfig::CONFIG_DISABLED_LINE_RANGE_MIN` amnesty branch of
//! `acceptable_range?` is not ported: it exists only to excuse the exact synthetic,
//! `-Infinity`-anchored range `inject_disabled_cops_directives` seeds for a cop the configuration
//! itself disables, and that seed is provably redundant with the *other* amnesty branch
//! (`registry.enabled?` is false and the range is still open at end-of-file) for every offense this
//! cop can ever report: a config-disabled cop's seeded range is closed only when a later `enable`
//! directive names it, which needs an explicit literal mention -- exactly the case this port's
//! per-literal-key grouping already handles the same way real registry expansion would, seed or no
//! seed (see [`acceptable`]'s doc comment for the full argument). Not seeding also means a
//! config-disabled cop that the file never mentions by name never gets a group at all, which is
//! moot: upstream's seeded-only range for such a cop is always acceptable too.

use std::collections::{BTreeMap, HashSet};

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, OptionValue,
    Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_directives::{CopRef, DirectiveKind};
use ruby_source::Span;

/// Checks that there is a `# rubocop:enable ...` after a `# rubocop:disable ...`.
#[derive(Debug, Clone)]
pub struct MissingCopEnableDirective {
    max_range: f64,
    options: RuleOptions,
}

/// Whether a finished disabled range's opening reference was a bare department word or a
/// slash-qualified cop path, mirroring upstream's `department_enabled?`/`type` local.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefKind {
    Cop,
    Department,
}

/// One literal reference's running disabled-range state, replayed across the whole directive
/// stream in source order -- RuboCop's `CommentConfig::CopAnalysis`, specialized to one already
/// literal-filtered directive stream instead of a registry-expanded one (see the module docs).
#[derive(Debug, Clone, Default)]
struct RangeState {
    ranges: Vec<(f64, f64)>,
    start: Option<f64>,
}

impl RangeState {
    /// Replays one directive's effect on this reference, mirroring `CommentConfig#analyze_cop`'s
    /// three branches (`analyze_single_line`/`analyze_disabled`/`analyze_rest`) plus the `-next`
    /// handling `ruby_directives::Directives::disabled_ranges_from` documents (this crate has no
    /// registry-driven statement-scope notion either, so `-next` still scopes to one physical
    /// line; see that method's doc comment for the full deviation).
    fn apply(&mut self, kind: DirectiveKind, line: u32, inline: bool) {
        let line = f64::from(line);
        if kind.is_next() {
            let target = line + 1.0;
            if kind.disables() {
                self.ranges.push((target, target));
            } else if let Some(s) = self.start {
                if target > s {
                    self.ranges.push((s, target - 1.0));
                }
                self.start = Some(target + 1.0);
            }
        } else if inline {
            if kind.disables() {
                self.ranges.push((line, line));
            }
        } else if kind.disables() {
            if let Some(s) = self.start {
                self.ranges.push((s, line));
            }
            self.start = Some(line);
        } else if let Some(s) = self.start.take() {
            self.ranges.push((s, line));
        }
    }

    /// Every finished range, including the still-open one (closed at end-of-file, `+Infinity`),
    /// mirroring `CommentConfig#cop_line_ranges`.
    fn into_ranges(mut self) -> Vec<(f64, f64)> {
        if let Some(s) = self.start {
            self.ranges.push((s, f64::INFINITY));
        }
        self.ranges
    }
}

/// The literal key one directive's cop reference is grouped under, plus how to describe it in a
/// message: `"all"` for `# rubocop:disable all` (an edge case with no real upstream fidelity
/// guarantee, since upstream's registry expansion would pick some specific, unpredictable real cop
/// name to name in the message instead), the bare department word for a department reference, or
/// the cop path as written for a specific cop.
fn describe(cop: &CopRef) -> (&str, RefKind) {
    match cop {
        CopRef::All => ("all", RefKind::Cop),
        CopRef::Department(name) => (name.as_str(), RefKind::Department),
        CopRef::Cop(name) => (name.as_str(), RefKind::Cop),
    }
}

/// `line_range.max - line_range.min < max_range + 2` plus the `registry.enabled?`-driven amnesty
/// for a cop the configuration itself disables (`acceptable_range?`); see the module docs for why
/// the third upstream branch (`line_range.min == CONFIG_DISABLED_LINE_RANGE_MIN`) needs no
/// equivalent here.
fn acceptable(min: f64, max: f64, max_range: f64, config_disabled: bool) -> bool {
    if max - min < max_range + 2.0 {
        return true;
    }
    config_disabled && max.is_infinite()
}

/// `Float#to_s`-like rendering for a whole `MaximumRangeSize` (RuboCop's own config values are
/// almost always plain integers, e.g. `2`; a genuinely fractional configured value renders with
/// its fractional part, matching `Kernel#format`'s `%s` on a Ruby `Float`).
fn format_max_range(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

/// The full comment on `line`, mirroring `ProcessedSource#comment_at_line`.
fn comment_at_line(ctx: &Context<'_>, line: u32) -> Option<Span> {
    ctx.comments().iter().find(|comment| comment.line == line).map(|comment| comment.span)
}

impl Rule for MissingCopEnableDirective {
    const META: RuleMeta = RuleMeta {
        name: "Lint/MissingCopEnableDirective",
        department: Department::Lint,
        summary: "Checks that there is a `# rubocop:enable ...` after a `# rubocop:disable ...`.",
        explanation: "\
Checks that there is an `# rubocop:enable ...` statement
after a `# rubocop:disable ...` statement. This will prevent leaving
cop disables on wide ranges of code, that latter contributors to
a file wouldn't be aware of.

You can set `MaximumRangeSize` to define the maximum number of
consecutive lines a cop can be disabled for.

- `.inf` any size (default)
- `0` allows only single-line disables
- `1` means the maximum allowed is as follows:

```ruby
# rubocop:disable SomeCop
a = 1
# rubocop:enable SomeCop
```

```ruby
# MaximumRangeSize: .inf (default)

# good
# rubocop:disable Layout/SpaceAroundOperators
x= 0
# rubocop:enable Layout/SpaceAroundOperators
# y = 1
# EOF

# bad
# rubocop:disable Layout/SpaceAroundOperators
x= 0
# EOF
```

```ruby
# MaximumRangeSize: 2

# good
# rubocop:disable Layout/SpaceAroundOperators
x= 0
# With the previous, there are 2 lines on which cop is disabled.
# rubocop:enable Layout/SpaceAroundOperators

# bad
# rubocop:disable Layout/SpaceAroundOperators
x= 0
x += 1
# Including this, that's 3 lines on which the cop is disabled.
# rubocop:enable Layout/SpaceAroundOperators
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[],
        config: &[ConfigOption {
            name: "MaximumRangeSize",
            default: ConfigDefault::Float(f64::INFINITY),
            allowed: &[],
            doc: "Maximum number of consecutive lines the cop can be disabled for.",
        }],
        blind_spots: "\
Ranges are grouped by each directive's own literal reference (`all`, a bare department word, or a
slash-qualified cop path) rather than by real, registry-expanded cop name -- see the module docs
for the full argument that this matches upstream for every case this cop's own spec covers, and
only diverges for the rare case a department-wide disable is later narrowed by re-enabling one of
its own specific members while leaving the rest disabled (upstream would still flag the other,
still-disabled members; this port considers the department reference closed as soon as anything
names it, department or member). `# rubocop:disable all` left open forever names `all` itself in
its message rather than upstream's unpredictable specific real cop name (registry expansion order),
an edge case with no meaningful upstream fidelity target.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let max_range = options
            .get("MaximumRangeSize")
            .and_then(OptionValue::as_float)
            .unwrap_or(f64::INFINITY);
        Ok(Self { max_range, options: options.clone() })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.check(ctx);
    }
}

impl MissingCopEnableDirective {
    fn check(&self, ctx: &mut Context<'_>) {
        let directives = ctx.directives().directives();
        if directives.is_empty() {
            return;
        }

        let mut states: BTreeMap<String, (RangeState, RefKind)> = BTreeMap::new();
        for directive in directives {
            for cop in &directive.cops {
                let (key, kind) = describe(cop);
                let entry =
                    states.entry(key.to_string()).or_insert_with(|| (RangeState::default(), kind));
                entry.0.apply(directive.kind, directive.line, directive.inline);
            }
        }

        let mut reported_spans: HashSet<Span> = HashSet::new();
        for (key, (state, kind)) in states {
            let config_disabled = self
                .options
                .peer(&key, "Enabled")
                .and_then(OptionValue::as_bool)
                .is_some_and(|enabled| !enabled);
            for (min, max) in state.into_ranges() {
                if acceptable(min, max, self.max_range, config_disabled) {
                    continue;
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let line = min as u32;
                let Some(span) = comment_at_line(ctx, line) else { continue };
                if !reported_spans.insert(span) {
                    continue;
                }
                ctx.report(&Self::META, span, self.message(&key, kind));
            }
        }
    }

    fn message(&self, cop: &str, kind: RefKind) -> String {
        let ty = match kind {
            RefKind::Department => "department",
            RefKind::Cop => "cop",
        };
        if self.max_range.is_infinite() {
            format!("Re-enable {cop} {ty} with `# rubocop:enable` after disabling it.")
        } else {
            format!(
                "Re-enable {cop} {ty} within {} lines after disabling it.",
                format_max_range(self.max_range)
            )
        }
    }
}
