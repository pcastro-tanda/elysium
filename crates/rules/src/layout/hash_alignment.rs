//! `Layout/HashAlignment`, ported from RuboCop's `lib/rubocop/cop/layout/hash_alignment.rb`
//! plus the `HashAlignmentStyles` mixin
//! (`lib/rubocop/cop/mixin/hash_alignment_styles.rb`) it includes.
//!
//! RuboCop keeps three little `Alignment` classes (`KeyAlignment`,
//! `TableAlignment`, `SeparatorAlignment`, plus `KeywordSplatAlignment` for
//! `**` entries) that each compute a `{key:, separator:, value:}` column
//! delta for a pair relative to the hash's first pair, tries every
//! configured style (`EnforcedHashRocketStyle`/`EnforcedColonStyle` can each
//! list more than one style), and keeps whichever style produced the fewest
//! offenses (ties keep the first-configured style). [`AlignKind`] plus the
//! free `key_deltas*`/`table_deltas*`/`separator_deltas*` functions below are
//! that port; [`check_pairs`] is `HashAlignment#check_pairs` plus
//! `#add_offenses`.
//!
//! Prism gives a braced hash literal its own [`NodeKind::HashNode`] and a
//! braceless bare-keyword-argument list its own [`NodeKind::KeywordHashNode`]
//! (RuboCop-AST's `hash_type?`/`braces?` split); both dispatch through
//! [`HashAlignment::on_hash`]. A pair's key keeps its trailing `:` inside its
//! own span for the label form (`foo:`), unlike whitequark's separate
//! operator token, so [`PairInfo::key_end_col_ws`] strips it back off
//! wherever RuboCop-AST would report a colon-free key length/column (e.g.
//! `TableAlignment`'s `max_key_width`); a right-hand-side `.=>`/`:`-agnostic
//! column *delta* between two same-kind keys does not need the adjustment
//! (the constant offset cancels), so [`generic_key_delta`]'s `Side::Right`
//! path is left alone. A value-shorthand pair (`{foo:}`) parses with an
//! [`NodeKind::ImplicitNode`] value whose span is exactly the key's own
//! span -- `PairInfo::value_omission` is `true` whenever the value is that
//! node kind, matching RuboCop-AST's `value_omission?` (`source.end_with?(':')`).
//!
//! `HashAlignment#autocorrect_incompatible_with_other_cops?` (skip a hash
//! whose first pair sits on the same line as `Layout/ArgumentAlignment`'s
//! `with_fixed_indentation` selector/left-sibling) and `#ignore_hash_argument?`
//! (`EnforcedLastArgumentHashStyle`, driven by `on_send`/`on_csend`/`on_super`/
//! `on_yield`) both need to know, while visiting a hash's enclosing call, what
//! that call's argument list looks like -- so [`HashAlignment::handle_call`]/
//! [`HashAlignment::handle_super_or_yield`] run when the engine enters the
//! *call* (before it descends into that call's hash argument) and record the
//! outcome by span in `incompatible_hashes`/`ignored_hashes`, consulted when
//! [`HashAlignment::on_hash`] later visits the hash itself. Both read
//! straight off the live `CallNode`/`SuperNode`/`YieldNode` handle the engine
//! hands `enter` -- no extra tree walk, just the single traversal's own
//! parent-before-child order.

use std::collections::{HashMap, HashSet};

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ArgumentsNode, AssocNode, CallNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MESSAGES[KeywordSplatAlignment]`.
const KWSPLAT_MSG: &str =
    "Align keyword splats with the rest of the hash if it spans more than one line.";

/// RuboCop's `HashAlignmentStyles`' three style classes: `key`/`table`/`separator`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AlignKind {
    Key,
    Table,
    Separator,
}

impl AlignKind {
    /// RuboCop's `MESSAGES[KeyAlignment | TableAlignment | SeparatorAlignment]`.
    const fn message(self) -> &'static str {
        match self {
            AlignKind::Key => "Align the keys of a hash literal if they span more than one line.",
            AlignKind::Separator => {
                "Align the separators of a hash literal if they span more than one line."
            }
            AlignKind::Table => {
                "Align the keys and values of a hash literal if they span more than one line."
            }
        }
    }
}

/// RuboCop's `EnforcedLastArgumentHashStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LastArgStyle {
    AlwaysInspect,
    AlwaysIgnore,
    IgnoreImplicit,
    IgnoreExplicit,
}

/// Left/right column comparison, RuboCop-AST's `HashElementNode#key_delta(alignment)`.
#[derive(Clone, Copy)]
enum Side {
    Left,
    Right,
}

/// A `{key:, separator:, value:}` column-delta map, RuboCop's per-pair delta
/// hash. `None` fields are simply absent from the Ruby hash (never checked);
/// an all-`None`/all-zero map is `good_alignment?` (no offense).
#[derive(Debug, Clone, Copy, Default)]
struct Delta {
    key: Option<i64>,
    separator: Option<i64>,
    value: Option<i64>,
}

impl Delta {
    fn is_good(self) -> bool {
        [self.key, self.separator, self.value].into_iter().flatten().all(|d| d == 0)
    }
}

/// Everything the alignment formulas need about one `AssocNode` pair, read
/// once off the live Prism node. Columns are 0-based character columns
/// (matching RuboCop's `Range#column`); `key_end_col_ws` and anything
/// derived from it are the whitequark-equivalent colon-free values (see the
/// module doc comment).
#[derive(Debug, Clone, Copy)]
struct PairInfo {
    span: Span,
    start_line: u32,
    end_line: u32,
    hash_rocket: bool,
    key_start: u32,
    key_col: u32,
    key_end_col_ws: u32,
    key_line: u32,
    value_start: u32,
    value_col: u32,
    value_line: u32,
    value_omission: bool,
    operator_start: u32,
    operator_col: u32,
    operator_end_col: u32,
    delimiter_width: u32,
}

/// Everything needed about one `AssocSplatNode` (`**foo`) element.
/// `KeywordSplatAlignment` treats the whole splat as its own "key".
#[derive(Debug, Clone, Copy)]
struct SplatInfo {
    span: Span,
    start_line: u32,
    end_line: u32,
    key_col: u32,
}

/// One child of a hash literal, RuboCop-AST's `pair_type?`/`kwsplat_type?` split.
#[derive(Debug, Clone, Copy)]
enum Elem {
    Pair(PairInfo),
    Splat(SplatInfo),
}

/// Looks for hash literals whose keys, separators, and values are aligned
/// according to the configured `EnforcedHashRocketStyle`/`EnforcedColonStyle`.
#[derive(Debug, Clone)]
pub struct HashAlignment {
    hash_rocket_styles: Vec<AlignKind>,
    colon_styles: Vec<AlignKind>,
    last_arg_style: LastArgStyle,
    /// `Layout/ArgumentAlignment`'s `EnforcedStyle == with_fixed_indentation`.
    fixed_indentation: bool,
    /// Spans of last-argument hashes `ignore_hash_argument?` says to skip
    /// (RuboCop's `ignore_node`), recorded when the engine visits the
    /// enclosing call/super/yield, before it descends into the hash itself.
    ignored_hashes: HashSet<Span>,
    /// Spans of hashes `autocorrect_incompatible_with_other_cops?` says to
    /// skip, recorded the same way.
    incompatible_hashes: HashSet<Span>,
}

impl Rule for HashAlignment {
    const META: RuleMeta = RuleMeta {
        name: "Layout/HashAlignment",
        department: Department::Layout,
        summary: "Align the elements of a hash literal if they span more than one line.",
        explanation: "\
Checks that the keys, separators, and values of a multi-line hash literal are
aligned according to the configured style.

```ruby
# EnforcedHashRocketStyle: key (default)
# bad
{
  :foo => bar,
   :ba => baz
}

# good
{
  :foo => bar,
  :ba => baz
}

# EnforcedHashRocketStyle: separator
# good
{
  :foo => bar,
   :ba => baz
}

# EnforcedHashRocketStyle: table
# good
{
  :foo => bar,
  :ba  => baz
}
```

The same three styles (`key`/`separator`/`table`) apply to colon-delimited
pairs via `EnforcedColonStyle`. Either option accepts a list of styles
instead of one; the style producing the fewest offenses wins.

`EnforcedLastArgumentHashStyle` controls whether a hash passed as the last
argument to a method call is inspected at all: `always_inspect` (default)
checks both implicit (`do_something(foo: 1,\\n  bar: 2)`) and explicit
(`do_something({foo: 1,\\n  bar: 2})`) hashes, `always_ignore` checks
neither, `ignore_implicit` skips only the braceless form, and
`ignore_explicit` skips only the braced form.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::SuperNode,
            NodeKind::YieldNode,
            NodeKind::HashNode,
            NodeKind::KeywordHashNode,
        ],
        config: &[
            ConfigOption {
                name: "EnforcedHashRocketStyle",
                default: ConfigDefault::Str("key"),
                allowed: &["key", "separator", "table"],
                doc: "Alignment of entries using a hash rocket (`=>`) as separator.",
            },
            ConfigOption {
                name: "EnforcedColonStyle",
                default: ConfigDefault::Str("key"),
                allowed: &["key", "separator", "table"],
                doc: "Alignment of entries using a colon (`:`) as separator.",
            },
            ConfigOption {
                name: "EnforcedLastArgumentHashStyle",
                default: ConfigDefault::Str("always_inspect"),
                allowed: &["always_inspect", "always_ignore", "ignore_implicit", "ignore_explicit"],
                doc: "Whether a hash passed as the last call argument is inspected.",
            },
            ConfigOption {
                name: "AllowMultipleStyles",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether `EnforcedHashRocketStyle`/`EnforcedColonStyle` may list several styles.",
            },
        ],
        blind_spots: "\
- `autocorrect_incompatible_with_other_cops?`'s `same_line?` override
  (`node1.last_line == line(node2)`) is ported exactly, but only for a hash
  whose *direct* parent is a plain call (`CallNode`); RuboCop itself never
  applies it to `super`/`yield` either (`call_type?` excludes both).
- Keys/values containing non-ASCII text use plain character counts (matching
  RuboCop's own `Range#column`, which is not display-width aware here
  either), so no East-Asian-width blind spot beyond RuboCop's own.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let hash_rocket_styles = parse_styles(options, "EnforcedHashRocketStyle");
        let colon_styles = parse_styles(options, "EnforcedColonStyle");
        let last_arg_style = match resolve_last_arg_style(options)? {
            "always_inspect" => LastArgStyle::AlwaysInspect,
            "always_ignore" => LastArgStyle::AlwaysIgnore,
            "ignore_implicit" => LastArgStyle::IgnoreImplicit,
            "ignore_explicit" => LastArgStyle::IgnoreExplicit,
            other => {
                return Err(options.error(
                    "EnforcedLastArgumentHashStyle",
                    format!("unsupported style `{other}`"),
                ));
            }
        };
        let fixed_indentation = options
            .peer("Layout/ArgumentAlignment", "EnforcedStyle")
            .and_then(OptionValue::as_str)
            .unwrap_or("with_first_argument")
            == "with_fixed_indentation";
        Ok(Self {
            hash_rocket_styles,
            colon_styles,
            last_arg_style,
            fixed_indentation,
            ignored_hashes: HashSet::new(),
            incompatible_hashes: HashSet::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                self.handle_call(&call, ctx);
            }
            NodeKind::SuperNode => {
                let sup = node.as_super_node().expect("kind matched");
                self.handle_super_or_yield(sup.arguments());
            }
            NodeKind::YieldNode => {
                let y = node.as_yield_node().expect("kind matched");
                self.handle_super_or_yield(y.arguments());
            }
            NodeKind::HashNode | NodeKind::KeywordHashNode => {
                self.on_hash(node, ctx);
            }
            _ => {}
        }
    }
}

impl HashAlignment {
    /// RuboCop's `on_send`/`on_csend` (`ignore_hash_argument?`) plus
    /// `autocorrect_incompatible_with_other_cops?`'s left-sibling/selector
    /// bookkeeping, both of which need this call's live argument list.
    fn handle_call(&mut self, call: &CallNode<'_>, ctx: &Context<'_>) {
        let Some(args_node) = call.arguments() else { return };
        let items: Vec<Node<'_>> = args_node.arguments().iter().collect();
        if let Some(last) = items.last() {
            self.check_ignore_last_arg(last);
        }
        if self.fixed_indentation {
            for (idx, item) in items.iter().enumerate() {
                if as_hash_like(item).is_some() {
                    self.check_fixed_indentation(call, &items, idx, item, ctx);
                }
            }
        }
    }

    /// RuboCop's `on_super`/`on_yield` (`ignore_hash_argument?` only --
    /// `autocorrect_incompatible_with_other_cops?` requires `call_type?`,
    /// which `super`/`yield` never satisfy).
    fn handle_super_or_yield(&mut self, args: Option<ArgumentsNode<'_>>) {
        let Some(args) = args else { return };
        let items: Vec<Node<'_>> = args.arguments().iter().collect();
        if let Some(last) = items.last() {
            self.check_ignore_last_arg(last);
        }
    }

    /// RuboCop's `ignore_hash_argument?`, applied to the call's last argument.
    fn check_ignore_last_arg(&mut self, last: &Node<'_>) {
        let Some(is_braced) = as_hash_like(last) else { return };
        let ignore = match self.last_arg_style {
            LastArgStyle::AlwaysInspect => false,
            LastArgStyle::AlwaysIgnore => true,
            LastArgStyle::IgnoreExplicit => is_braced,
            LastArgStyle::IgnoreImplicit => !is_braced,
        };
        if ignore {
            self.ignored_hashes.insert(last.span());
        }
    }

    /// RuboCop's `autocorrect_incompatible_with_other_cops?` plus
    /// `argument_before_hash`/`same_line?`, evaluated for `items[idx]`
    /// (already known to be a hash-like node).
    fn check_fixed_indentation(
        &mut self,
        call: &CallNode<'_>,
        items: &[Node<'_>],
        idx: usize,
        hash_node: &Node<'_>,
        ctx: &Context<'_>,
    ) {
        let elements = hash_elements(hash_node);
        let Some(first_pair) = elements.iter().find(|e| e.as_assoc_node().is_some()) else {
            return;
        };
        let first_pair_start_line = ctx.line_col(first_pair.span().start).line;

        // `argument_before_hash`: the hash's own leading kwsplat's inner
        // value takes priority over the call's true previous argument.
        let reference: Option<Span> = elements
            .first()
            .and_then(ruby_ast::Node::as_assoc_splat_node)
            .and_then(|s| s.value())
            .map(|v| v.span())
            .or_else(|| if idx > 0 { Some(items[idx - 1].span()) } else { None });
        let selector_span = reference.unwrap_or_else(|| {
            call.message_loc().map_or_else(|| call.location().span(), |l| l.span())
        });

        let ref_start_line = ctx.line_col(selector_span.start).line;
        let ref_end_line = ctx.line_col(last_offset(selector_span)).line;
        if ref_start_line == first_pair_start_line || ref_end_line == first_pair_start_line {
            self.incompatible_hashes.insert(hash_node.span());
        }
    }

    /// RuboCop's `on_hash`.
    fn on_hash(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let span = node.span();
        if self.incompatible_hashes.contains(&span) || self.ignored_hashes.contains(&span) {
            return;
        }
        if is_single_line(span, ctx) {
            return;
        }

        let raw = hash_elements(node);
        let elems: Vec<Elem> = raw
            .iter()
            .map(|n| {
                n.as_assoc_node().map_or_else(
                    || Elem::Splat(build_splat_info(n, ctx)),
                    |p| Elem::Pair(build_pair_info(&p, ctx)),
                )
            })
            .collect();

        let pairs: Vec<&PairInfo> = elems
            .iter()
            .filter_map(|e| if let Elem::Pair(p) = e { Some(p) } else { None })
            .collect();
        if pairs.is_empty() {
            return;
        }

        let rocket_ok = self.hash_rocket_styles.iter().any(|&k| checkable_layout(k, &pairs));
        let colon_ok = self.colon_styles.iter().any(|&k| checkable_layout(k, &pairs));
        if !rocket_ok || !colon_ok {
            return;
        }

        for (offense_span, message, fix) in self.check_pairs(&elems, ctx) {
            match fix {
                Some(f) => ctx.report_with_fix(&Self::META, offense_span, message, f),
                None => ctx.report(&Self::META, offense_span, message),
            }
        }
    }

    /// RuboCop's `check_pairs` plus `add_offenses`/`register_offenses_with_format`.
    fn check_pairs(
        &self,
        elems: &[Elem],
        ctx: &Context<'_>,
    ) -> Vec<(Span, &'static str, Option<Fix>)> {
        let pairs: Vec<&PairInfo> = elems
            .iter()
            .filter_map(|e| if let Elem::Pair(p) = e { Some(p) } else { None })
            .collect();
        let fp_index = elems
            .iter()
            .position(|e| matches!(e, Elem::Pair(_)))
            .expect("caller checked non-empty");
        let Elem::Pair(fp) = elems[fp_index] else { unreachable!() };
        let mkw = max_key_width(&pairs);
        let mdw = max_delimiter_width(&pairs);

        let mut order: Vec<AlignKind> = Vec::new();
        let mut push_count: HashMap<AlignKind, usize> = HashMap::new();
        let mut latest: HashMap<AlignKind, HashMap<usize, Delta>> = HashMap::new();

        // Stage 1: `alignment_for(first_pair).each { deltas_for_first_pair }`.
        for &kind in styles_for(self, fp.hash_rocket) {
            ensure_bucket(kind, &mut order, &mut push_count);
            let delta = match kind {
                AlignKind::Key => key_deltas_for_first(&fp),
                AlignKind::Table => table_deltas_for_first(&fp, mkw, mdw),
                AlignKind::Separator => Delta::default(),
            };
            record(kind, fp_index, delta, &mut push_count, &mut latest);
        }

        // Stage 2: `node.children.each { alignment_for(current).each { deltas } }`.
        let mut kwsplat_hits: Vec<(usize, Delta)> = Vec::new();
        for (idx, elem) in elems.iter().enumerate() {
            match elem {
                Elem::Pair(current) => {
                    for &kind in styles_for(self, current.hash_rocket) {
                        ensure_bucket(kind, &mut order, &mut push_count);
                        let delta = match kind {
                            AlignKind::Key => key_deltas(&fp, current, ctx),
                            AlignKind::Table => table_deltas(&fp, current, mkw, mdw),
                            AlignKind::Separator => separator_deltas(&fp, current),
                        };
                        record(kind, idx, delta, &mut push_count, &mut latest);
                    }
                }
                Elem::Splat(splat) => {
                    let delta = kwsplat_delta(&fp, splat, ctx);
                    if !delta.is_good() {
                        kwsplat_hits.push((idx, delta));
                    }
                }
            }
        }

        let mut out: Vec<(Span, &'static str, Option<Fix>)> = Vec::new();
        // `KeywordSplatAlignment` offenses are always reported, independent
        // of which other format wins.
        for (idx, delta) in kwsplat_hits {
            let Elem::Splat(s) = elems[idx] else { unreachable!() };
            let mut edits = Vec::new();
            adjust_edit(ctx, delta.key.unwrap_or(0), s.span.start, &mut edits);
            let fix =
                (!edits.is_empty()).then_some(Fix { applicability: Applicability::Safe, edits });
            out.push((s.span, KWSPLAT_MSG, fix));
        }

        // `offenses_by.min_by { |_, v| v.length }`: first-inserted format wins ties.
        if let Some(winner) = order.iter().copied().min_by_key(|k| push_count[k]) {
            if let Some(map) = latest.get(&winner) {
                for &idx in map.keys() {
                    let Elem::Pair(p) = elems[idx] else { unreachable!() };
                    // RuboCop's `register_offenses_with_format`: the fix for
                    // each offense uses `alignment_for(offense).first`'s own
                    // recorded delta -- the *pair's own* first-preferred
                    // style for its delimiter kind -- not necessarily the
                    // winning `format` whose message is shown. If that
                    // style's delta for this node was never recorded (i.e.
                    // it was already `good_alignment?`), no fix is queued.
                    let first_style = if p.hash_rocket {
                        self.hash_rocket_styles[0]
                    } else {
                        self.colon_styles[0]
                    };
                    let fix_delta = latest.get(&first_style).and_then(|m| m.get(&idx)).copied();
                    let fix = fix_delta
                        .map(|d| build_pair_fix(ctx, &p, d))
                        .filter(|f| !f.edits.is_empty());
                    out.push((p.span, winner.message(), fix));
                }
            }
        }

        out
    }
}

/// `push_count`/`order` bootstrap, RuboCop's `offenses_by[alignment.class] ||= []`
/// (first-seen order tracked separately since `HashMap` does not preserve it).
fn ensure_bucket(
    kind: AlignKind,
    order: &mut Vec<AlignKind>,
    push_count: &mut HashMap<AlignKind, usize>,
) {
    push_count.entry(kind).or_insert_with(|| {
        order.push(kind);
        0
    });
}

/// RuboCop's `check_delta`: records a push (for the `min_by` tie-break) and
/// the latest delta (for the eventual fix) only when the delta is not
/// `good_alignment?`.
fn record(
    kind: AlignKind,
    idx: usize,
    delta: Delta,
    push_count: &mut HashMap<AlignKind, usize>,
    latest: &mut HashMap<AlignKind, HashMap<usize, Delta>>,
) {
    if delta.is_good() {
        return;
    }
    *push_count.get_mut(&kind).expect("bucket already ensured") += 1;
    latest.entry(kind).or_default().insert(idx, delta);
}

fn styles_for(rule: &HashAlignment, hash_rocket: bool) -> &[AlignKind] {
    if hash_rocket {
        &rule.hash_rocket_styles
    } else {
        &rule.colon_styles
    }
}

/// RuboCop-AST's `HashNode#pairs`/`#elements`: every `AssocNode`/`AssocSplatNode`
/// child, for either hash flavor Prism produces (see the module doc comment).
fn hash_elements<'pr>(node: &Node<'pr>) -> Vec<Node<'pr>> {
    match node.kind() {
        NodeKind::HashNode => {
            node.as_hash_node().expect("kind matched").elements().iter().collect()
        }
        NodeKind::KeywordHashNode => {
            node.as_keyword_hash_node().expect("kind matched").elements().iter().collect()
        }
        _ => Vec::new(),
    }
}

/// RuboCop-AST's `hash_type?` (covers both `HashNode` and `KeywordHashNode`)
/// plus `braces?`; `None` when `node` is not a hash at all.
fn as_hash_like(node: &Node<'_>) -> Option<bool> {
    if node.as_hash_node().is_some() {
        Some(true)
    } else if node.as_keyword_hash_node().is_some() {
        Some(false)
    } else {
        None
    }
}

/// An `EnforcedStyle`-like list option: RuboCop's `new_alignment` (a lone
/// style string or a list of styles, deduplicated, unknown entries dropped
/// -- false negatives over guessing).
fn parse_styles(options: &RuleOptions, key: &str) -> Vec<AlignKind> {
    let mut out = Vec::new();
    for raw in options.str_list(key) {
        let kind = match raw.as_str() {
            "key" => AlignKind::Key,
            "table" => AlignKind::Table,
            "separator" => AlignKind::Separator,
            _ => continue,
        };
        if !out.contains(&kind) {
            out.push(kind);
        }
    }
    if out.is_empty() {
        out.push(AlignKind::Key);
    }
    out
}

/// An `EnforcedStyle`-like option that tolerates an explicit YAML `~`
/// (RuboCop's spec fixtures sometimes write this for "no override", which
/// [`RuleOptions::style`] would otherwise reject as a non-string value).
fn resolve_last_arg_style(options: &RuleOptions) -> Result<&str, OptionError> {
    if matches!(options.get("EnforcedLastArgumentHashStyle"), Some(OptionValue::Null)) {
        return Ok("always_inspect");
    }
    options.style("EnforcedLastArgumentHashStyle")
}

fn build_pair_info(pair: &AssocNode<'_>, ctx: &Context<'_>) -> PairInfo {
    let span = pair.location().span();
    let start_line = ctx.line_col(span.start).line;
    let end_line = ctx.line_col(last_offset(span)).line;

    let key = pair.key();
    let key_span = key.location().span();
    let hash_rocket = pair.operator_loc().is_some();
    let key_col = ctx.line_col(key_span.start).column;
    let key_line = ctx.line_col(key_span.start).line;
    let key_end_col_raw = ctx.line_col(key_span.end).column;
    let key_end_col_ws =
        if hash_rocket { key_end_col_raw } else { key_end_col_raw.saturating_sub(1) };

    let value = pair.value();
    let value_omission = value.as_implicit_node().is_some();
    let value_span = value.location().span();
    let value_col = ctx.line_col(value_span.start).column;
    let value_line = ctx.line_col(value_span.start).line;

    let (operator_start, operator_col, operator_end_col) = if let Some(op_loc) = pair.operator_loc()
    {
        let s = op_loc.span();
        (s.start, ctx.line_col(s.start).column, ctx.line_col(s.end).column)
    } else {
        let colon_start = key_span.end.saturating_sub(1);
        (colon_start, key_end_col_ws, key_end_col_ws + 1)
    };
    let delimiter_width = if hash_rocket { 4 } else { 2 };

    PairInfo {
        span,
        start_line,
        end_line,
        hash_rocket,
        key_start: key_span.start,
        key_col,
        key_end_col_ws,
        key_line,
        value_start: value_span.start,
        value_col,
        value_line,
        value_omission,
        operator_start,
        operator_col,
        operator_end_col,
        delimiter_width,
    }
}

fn build_splat_info(node: &Node<'_>, ctx: &Context<'_>) -> SplatInfo {
    let span = node.span();
    SplatInfo {
        span,
        start_line: ctx.line_col(span.start).line,
        end_line: ctx.line_col(last_offset(span)).line,
        key_col: ctx.line_col(span.start).column,
    }
}

fn last_offset(span: Span) -> u32 {
    if span.end > span.start {
        span.end - 1
    } else {
        span.start
    }
}

fn is_single_line(span: Span, ctx: &Context<'_>) -> bool {
    ctx.line_col(span.start).line == ctx.line_col(last_offset(span)).line
}

/// RuboCop-AST's `HashElementNode#same_line?`.
fn same_line_pair(a: &PairInfo, b: &PairInfo) -> bool {
    a.end_line == b.start_line || a.start_line == b.end_line
}

fn pairs_on_same_line(pairs: &[&PairInfo]) -> bool {
    pairs.windows(2).any(|w| same_line_pair(w[0], w[1]))
}

fn mixed_delimiters(pairs: &[&PairInfo]) -> bool {
    pairs.iter().any(|p| p.hash_rocket) && pairs.iter().any(|p| !p.hash_rocket)
}

/// RuboCop's `KeyAlignment#checkable_layout?` (always true) and
/// `ValueAlignment#checkable_layout?` (`Table`/`Separator`).
fn checkable_layout(kind: AlignKind, pairs: &[&PairInfo]) -> bool {
    match kind {
        AlignKind::Key => true,
        AlignKind::Table | AlignKind::Separator => {
            !pairs_on_same_line(pairs) && !mixed_delimiters(pairs)
        }
    }
}

fn max_key_width(pairs: &[&PairInfo]) -> u32 {
    pairs.iter().map(|p| p.key_end_col_ws - p.key_col).max().unwrap_or(0)
}

fn max_delimiter_width(pairs: &[&PairInfo]) -> u32 {
    pairs.iter().map(|p| p.delimiter_width).max().unwrap_or(0)
}

/// RuboCop-AST's `HashElementNode#key_delta`.
fn generic_key_delta(fp: &PairInfo, current: &PairInfo, side: Side) -> i64 {
    if same_line_pair(fp, current) {
        return 0;
    }
    match side {
        Side::Left => i64::from(fp.key_col) - i64::from(current.key_col),
        Side::Right => i64::from(fp.key_end_col_ws) - i64::from(current.key_end_col_ws),
    }
}

/// RuboCop-AST's `HashElementNode#value_delta`.
fn generic_value_delta(fp: &PairInfo, current: &PairInfo) -> i64 {
    if same_line_pair(fp, current) {
        return 0;
    }
    i64::from(fp.value_col) - i64::from(current.value_col)
}

/// RuboCop-AST's `HashElementNode#delimiter_delta`.
fn generic_delimiter_delta(fp: &PairInfo, current: &PairInfo) -> i64 {
    if same_line_pair(fp, current) || fp.hash_rocket != current.hash_rocket {
        return 0;
    }
    i64::from(fp.operator_col) - i64::from(current.operator_col)
}

/// `KeyAlignment#deltas_for_first_pair`.
fn key_deltas_for_first(fp: &PairInfo) -> Delta {
    Delta { key: None, separator: Some(key_separator_delta(fp)), value: Some(key_value_delta(fp)) }
}

/// `KeyAlignment#deltas`.
fn key_deltas(fp: &PairInfo, current: &PairInfo, ctx: &Context<'_>) -> Delta {
    if !begins_its_line(ctx, current.span) {
        return Delta::default();
    }
    let key = if same_line_pair(fp, current) {
        0
    } else {
        i64::from(fp.key_col) - i64::from(current.key_col)
    };
    Delta {
        key: Some(key),
        separator: Some(key_separator_delta(current)),
        value: Some(key_value_delta(current)),
    }
}

fn key_separator_delta(p: &PairInfo) -> i64 {
    if p.hash_rocket {
        i64::from(p.key_end_col_ws) + 1 - i64::from(p.operator_col)
    } else {
        0
    }
}

fn key_value_delta(p: &PairInfo) -> i64 {
    if p.value_line != p.key_line || p.value_omission {
        return 0;
    }
    i64::from(p.operator_end_col) + 1 - i64::from(p.value_col)
}

/// `TableAlignment#deltas_for_first_pair`.
fn table_deltas_for_first(fp: &PairInfo, mkw: u32, mdw: u32) -> Delta {
    let sep = if fp.hash_rocket { table_hash_rocket_delta(fp, fp, mkw) } else { 0 };
    let val = table_value_delta_priv(fp, fp, mkw, mdw) - sep;
    Delta { key: None, separator: Some(sep), value: Some(val) }
}

/// `TableAlignment#deltas` (via `ValueAlignment#deltas`).
fn table_deltas(fp: &PairInfo, current: &PairInfo, mkw: u32, mdw: u32) -> Delta {
    let key = generic_key_delta(fp, current, Side::Left);
    let sep = if current.hash_rocket { table_hash_rocket_delta(fp, current, mkw) - key } else { 0 };
    let val = table_value_delta_priv(fp, current, mkw, mdw) - key - sep;
    Delta { key: Some(key), separator: Some(sep), value: Some(val) }
}

fn table_hash_rocket_delta(fp: &PairInfo, current: &PairInfo, mkw: u32) -> i64 {
    i64::from(fp.key_col) + i64::from(mkw) + 1 - i64::from(current.operator_col)
}

fn table_value_delta_priv(fp: &PairInfo, current: &PairInfo, mkw: u32, mdw: u32) -> i64 {
    if current.value_omission {
        return 0;
    }
    i64::from(fp.key_col) + i64::from(mkw) + i64::from(mdw) - i64::from(current.value_col)
}

/// `SeparatorAlignment#deltas` (via `ValueAlignment#deltas`;
/// `#deltas_for_first_pair` is always `{}`, i.e. [`Delta::default`]).
fn separator_deltas(fp: &PairInfo, current: &PairInfo) -> Delta {
    let key = generic_key_delta(fp, current, Side::Right);
    let sep = if current.hash_rocket { generic_delimiter_delta(fp, current) - key } else { 0 };
    let val = separator_value_delta_priv(fp, current) - key - sep;
    Delta { key: Some(key), separator: Some(sep), value: Some(val) }
}

fn separator_value_delta_priv(fp: &PairInfo, current: &PairInfo) -> i64 {
    if current.value_omission {
        return 0;
    }
    generic_value_delta(fp, current)
}

/// `KeywordSplatAlignment#deltas`.
fn kwsplat_delta(fp: &PairInfo, splat: &SplatInfo, ctx: &Context<'_>) -> Delta {
    if !begins_its_line(ctx, splat.span) {
        return Delta::default();
    }
    let same_line = fp.end_line == splat.start_line || fp.start_line == splat.end_line;
    let key = if same_line { 0 } else { i64::from(fp.key_col) - i64::from(splat.key_col) };
    Delta { key: Some(key), separator: None, value: None }
}

/// RuboCop's `correct_node`/`correct_key_value`/`correct_no_value`.
fn build_pair_fix(ctx: &Context<'_>, pair: &PairInfo, delta: Delta) -> Fix {
    let mut edits = Vec::new();
    if pair.value_omission {
        adjust_edit(ctx, delta.key.unwrap_or(0), pair.key_start, &mut edits);
    } else {
        let key_col = i64::from(pair.key_col);
        let key_delta = delta.key.unwrap_or(0).max(-key_col);
        adjust_edit(ctx, key_delta, pair.key_start, &mut edits);
        adjust_edit(ctx, delta.separator.unwrap_or(0), pair.operator_start, &mut edits);
        adjust_edit(ctx, delta.value.unwrap_or(0), pair.value_start, &mut edits);
    }
    Fix { applicability: Applicability::Safe, edits }
}

/// RuboCop's `adjust`: inserts `delta` spaces before `target`, or removes
/// `-delta` characters immediately preceding it.
fn adjust_edit(ctx: &Context<'_>, delta: i64, target: u32, edits: &mut Vec<Edit>) {
    if delta > 0 {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let spaces = vec![b' '; delta as usize];
        edits.push(Edit::insert(target, spaces));
    } else if delta < 0 {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let n = (-delta) as u32;
        let from = char_offset_before(ctx, target, n);
        if from < target {
            edits.push(Edit::delete(Span::new(from, target)));
        }
    }
}

/// The byte offset `n` characters before `point`, never crossing the start
/// of `point`'s own line.
fn char_offset_before(ctx: &Context<'_>, point: u32, n: u32) -> u32 {
    if n == 0 {
        return point;
    }
    let line = ctx.line_col(point).line;
    let line_start = ctx.line_span(line).start;
    let prefix = ctx.text(Span::new(line_start, point));
    let mut idx = prefix.len();
    let mut remaining = n;
    while idx > 0 && remaining > 0 {
        idx -= 1;
        while idx > 0 && (prefix[idx] & 0xC0) == 0x80 {
            idx -= 1;
        }
        remaining -= 1;
    }
    line_start + u32::try_from(idx).unwrap_or(0)
}

/// RuboCop's `Util#begins_its_line?`, character-based (matching Ruby's
/// `String#index`/`Range#column`) so it also holds on lines with non-ASCII
/// leading content.
fn begins_its_line(ctx: &Context<'_>, span: Span) -> bool {
    let line_col = ctx.line_col(span.start);
    let line = ctx.line_text(line_col.line);
    let Ok(text) = std::str::from_utf8(line) else { return line_col.column == 0 };
    match text.chars().position(|ch| !is_ruby_whitespace(ch)) {
        Some(index) => u32::try_from(index).unwrap_or(u32::MAX) == line_col.column,
        None => false,
    }
}

/// Ruby's `\s` character class, used by `begins_its_line?`'s regex.
fn is_ruby_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\r' | '\x0B' | '\x0C')
}
