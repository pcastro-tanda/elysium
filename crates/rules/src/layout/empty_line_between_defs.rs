//! `Layout/EmptyLineBetweenDefs`, ported from RuboCop's
//! `lib/rubocop/cop/layout/empty_line_between_defs.rb`.
//!
//! RuboCop drives this cop off `on_begin`, walking consecutive pairs of a
//! `begin` node's children; Prism's [`ruby_ast::node::StatementsNode`] is
//! this cop's operative "begin" (every multi-statement body -- program,
//! class/module/def/block body, an `if`/`unless` branch, ...), so this rule
//! subscribes to [`NodeKind::StatementsNode`] and scans its direct
//! `body()` list pairwise -- no extra tree walk, [`Context::ancestors`]
//! gives the statements list's own enclosing scope for free when it is
//! entered.
//!
//! A candidate pair member is a method definition ([`DefNode`]), a
//! `class`/`module` definition, or (when configured via `DefLikeMacros`) a
//! receiver-less call that RuboCop's `macro?` would accept -- approximated
//! here as "the statements list's enclosing scope, skipping transparent
//! `if`/`unless`/`begin`/block wrappers, bottoms out at a class, module,
//! singleton class, or the program root" (RuboCop-AST's `in_macro_scope?`
//! node pattern, translated one-for-one since Prism has no synthetic
//! `begin`/`kwbegin` wrapper for a bare statements list).

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::Node;
use ruby_ast::{LocationExt as _, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG_TEMPLATE: (&str, &str, &str) = ("Expected ", " between ", " definitions; found ");

/// What kind of definition a candidate node is; also RuboCop's `node_type`
/// message label (`:block`/`:numblock`/`:itblock` all collapse to
/// `"block"`, everything else uses the node's own type name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Method,
    Class,
    Module,
    Send,
    Block,
}

impl Kind {
    const fn label(self) -> &'static str {
        match self {
            Kind::Method => "method",
            Kind::Class => "class",
            Kind::Module => "module",
            Kind::Send => "send",
            Kind::Block => "block",
        }
    }
}

/// One candidate member of a consecutive pair: RuboCop's `candidate?`
/// already resolved, plus the facts `check_defs`/`def_location` need.
struct Candidate {
    kind: Kind,
    /// The node's own full span (RuboCop's `def_start`/`def_end`/`end_pos`
    /// all reduce to this span's start/end line, since the `keyword`/
    /// `send_node` position used for "which line does this start on" is
    /// always the node's own first token).
    span: Span,
    /// RuboCop's `single_line?`.
    single_line: bool,
    /// RuboCop's `def_location`: the offense range when this candidate is
    /// the correction node (`nodes.last`).
    location: Span,
}

/// The `EmptyLineBetween{Method,Class,Module}Defs` gates, grouped so the
/// containing rule struct does not trip clippy's bool-field limit.
#[derive(Debug, Clone, Copy)]
struct EnabledFor {
    method: bool,
    class: bool,
    module: bool,
}

/// Checks whether class/module/method definitions are separated by one or
/// more empty lines.
#[derive(Debug, Clone)]
pub struct EmptyLineBetweenDefs {
    enabled_for: EnabledFor,
    allow_adjacent_one_line_defs: bool,
    def_like_macros: Vec<Vec<u8>>,
    min_lines: i64,
    max_lines: i64,
}

impl EmptyLineBetweenDefs {
    /// RuboCop's `expected_lines`.
    fn expected_lines(&self) -> String {
        if self.min_lines != self.max_lines {
            format!("{}..{} empty lines", self.min_lines, self.max_lines)
        } else if self.max_lines == 1 {
            "1 empty line".to_string()
        } else {
            format!("{} empty lines", self.max_lines)
        }
    }

    /// RuboCop's `line_count_allowed?`.
    const fn line_count_allowed(&self, count: i64) -> bool {
        count >= self.min_lines && count <= self.max_lines
    }

    /// Classifies one `StatementsNode` child as a candidate pair member,
    /// applying the `EmptyLineBetween*Defs`/`DefLikeMacros` gates. `None`
    /// for anything else (RuboCop's `candidate?` returning false), which
    /// also breaks the consecutive-pair chain around it.
    fn classify(
        &self,
        node: &Node<'_>,
        ctx: &Context<'_>,
        in_macro_scope: bool,
    ) -> Option<Candidate> {
        let span = node.span();
        let single_line = ctx.is_single_line(span);

        if let Some(def) = node.as_def_node() {
            if !self.enabled_for.method {
                return None;
            }
            let location = def.def_keyword_loc().span().join(def.name_loc().span());
            return Some(Candidate { kind: Kind::Method, span, single_line, location });
        }
        if let Some(class) = node.as_class_node() {
            if !self.enabled_for.class {
                return None;
            }
            let location = class.class_keyword_loc().span().join(class.constant_path().span());
            return Some(Candidate { kind: Kind::Class, span, single_line, location });
        }
        if let Some(module) = node.as_module_node() {
            if !self.enabled_for.module {
                return None;
            }
            let location = module.module_keyword_loc().span().join(module.constant_path().span());
            return Some(Candidate { kind: Kind::Module, span, single_line, location });
        }
        if let Some(call) = node.as_call_node() {
            if call.receiver().is_some() || !in_macro_scope {
                return None;
            }
            let name = call.name();
            if !self.def_like_macros.iter().any(|m| m.as_slice() == name.as_slice()) {
                return None;
            }
            let kind = if call.block().is_some() { Kind::Block } else { Kind::Send };
            return Some(Candidate { kind, span, single_line, location: span });
        }
        None
    }

    /// Checks one consecutive candidate pair: RuboCop's `check_defs`.
    fn check_pair(&self, prev: &Candidate, cur: &Candidate, ctx: &mut Context<'_>) {
        let count = blank_lines_between(ctx, prev.span, cur.span);
        if self.line_count_allowed(count) {
            return;
        }
        if multiple_blank_line_groups(ctx, prev.span, cur.span) {
            return;
        }
        if prev.single_line && cur.single_line && self.allow_adjacent_one_line_defs {
            return;
        }

        let (prefix, mid, suffix) = MSG_TEMPLATE;
        let message =
            format!("{prefix}{}{mid}{}{suffix}{count}.", self.expected_lines(), cur.kind.label());
        let fix = self.build_fix(ctx, prev.span, cur.span, count);
        ctx.report_with_fix(&Self::META, cur.location, message, fix);
    }

    /// RuboCop's `autocorrect`.
    fn build_fix(&self, ctx: &Context<'_>, prev_span: Span, cur_span: Span, count: i64) -> Fix {
        let bytes = ctx.source().bytes();
        let end_pos = prev_span.end;
        let begin_pos = cur_span.start;
        let found_newline = find_byte(bytes, b'\n', end_pos);
        // RuboCop's "handle the case when multiple one-liners are on the
        // same line": no real newline separates the pair (or none exists
        // at all past `prev`), so anchor right before `cur` instead.
        let anchor = match found_newline {
            Some(pos) if pos <= begin_pos => pos,
            _ => begin_pos.saturating_sub(1),
        };

        if count > self.max_lines {
            let difference = u32::try_from(count - self.max_lines).unwrap_or(0);
            let span = Span::new(anchor, anchor.saturating_add(difference));
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(span)] }
        } else {
            let difference = u32::try_from(self.min_lines - count).unwrap_or(0);
            let text = "\n".repeat(difference as usize);
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::insert(anchor.saturating_add(1), text.into_bytes())],
            }
        }
    }
}

/// First `\n` at or after `from`, if any.
fn find_byte(bytes: &[u8], needle: u8, from: u32) -> Option<u32> {
    let from = from as usize;
    if from >= bytes.len() {
        return None;
    }
    bytes[from..]
        .iter()
        .position(|&b| b == needle)
        .map(|i| u32::try_from(from + i).unwrap_or(u32::MAX))
}

/// A string that is empty or contains only whitespace: RuboCop's core-ext
/// `String#blank?` as applied to one physical line.
fn is_blank_line(ctx: &Context<'_>, line: u32) -> bool {
    ctx.line_text(line).iter().all(u8::is_ascii_whitespace)
}

/// The inclusive 1-based line range strictly between `first_span`'s end
/// line and `second_span`'s start line, or `None` when they are adjacent
/// or share a line (RuboCop's `lines_between_defs` returning `[]`).
fn gap_lines(ctx: &Context<'_>, first_span: Span, second_span: Span) -> Option<(u32, u32)> {
    let end_line = ctx.last_line(first_span);
    let start_line = ctx.line_col(second_span.start).line;
    let gap_first = end_line + 1;
    let gap_last = start_line.saturating_sub(1);
    (gap_first <= gap_last).then_some((gap_first, gap_last))
}

/// RuboCop's `blank_lines_count_between`.
fn blank_lines_between(ctx: &Context<'_>, first_span: Span, second_span: Span) -> i64 {
    match gap_lines(ctx, first_span, second_span) {
        Some((first, last)) => {
            let count = (first..=last).filter(|&line| is_blank_line(ctx, line)).count();
            i64::try_from(count).unwrap_or(i64::MAX)
        }
        None => 0,
    }
}

/// RuboCop's `multiple_blank_lines_groups?`: true when a blank line occurs
/// strictly after some non-blank (comment) line within the gap, i.e. the
/// gap is not a single contiguous blank-line run followed by (or preceded
/// by) comments.
fn multiple_blank_line_groups(ctx: &Context<'_>, first_span: Span, second_span: Span) -> bool {
    let Some((first, last)) = gap_lines(ctx, first_span, second_span) else { return false };
    let mut blank_max = None;
    let mut non_blank_min = None;
    for line in first..=last {
        if is_blank_line(ctx, line) {
            blank_max = Some(line);
        } else if non_blank_min.is_none() {
            non_blank_min = Some(line);
        }
    }
    matches!((blank_max, non_blank_min), (Some(blank), Some(non_blank)) if blank > non_blank)
}

/// RuboCop-AST's `in_macro_scope?`, approximated over [`Context::ancestors`]:
/// walking outward from the statements list's own parent, `if`/`unless`/
/// `begin`/block wrappers are transparent; a class, module, singleton
/// class, or the program root grants macro scope; anything else (a `def`
/// body, a `case`/`while` branch, ...) does not.
fn in_macro_scope(ancestors: &[linter::NodeInfo]) -> bool {
    for info in ancestors.iter().rev() {
        match info.kind {
            NodeKind::IfNode | NodeKind::UnlessNode | NodeKind::BeginNode | NodeKind::BlockNode => {
            }
            NodeKind::ClassNode
            | NodeKind::ModuleNode
            | NodeKind::SingletonClassNode
            | NodeKind::ProgramNode => return true,
            _ => return false,
        }
    }
    true
}

impl Rule for EmptyLineBetweenDefs {
    const META: RuleMeta = RuleMeta {
        name: "Layout/EmptyLineBetweenDefs",
        department: Department::Layout,
        summary: "Use empty lines between class/module/method defs.",
        explanation: "\
Checks whether class/module/method definitions are separated by one or more \
empty lines.

`NumberOfEmptyLines` can be an integer (default is 1) or an array (e.g. \
`[1, 2]`) to specify a minimum and maximum number of empty lines permitted.

`AllowAdjacentOneLineDefs` configures whether adjacent one-line definitions \
are considered an offense.

```ruby
# bad
def a
end
def b
end

# good
def a
end

def b
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StatementsNode],
        config: &[
            ConfigOption {
                name: "EmptyLineBetweenMethodDefs",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Checks for empty lines between method definitions.",
            },
            ConfigOption {
                name: "EmptyLineBetweenClassDefs",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Checks for empty lines between class definitions.",
            },
            ConfigOption {
                name: "EmptyLineBetweenModuleDefs",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Checks for empty lines between module definitions.",
            },
            ConfigOption {
                name: "DefLikeMacros",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "The name of any macro that you want to treat like a def.",
            },
            ConfigOption {
                name: "AllowAdjacentOneLineDefs",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether single line method definitions need an empty line between them.",
            },
            ConfigOption {
                name: "NumberOfEmptyLines",
                default: ConfigDefault::Int(1),
                allowed: &[],
                doc: "Can be an array to specify a minimum and maximum number of empty lines, \
                      e.g. `[1, 2]`.",
            },
        ],
        blind_spots: "\
`DefLikeMacros` candidacy approximates RuboCop-AST's `in_macro_scope?` (a \
receiver-less call sits in a class/module/singleton-class body, or at the \
program's top level, tunneling through `if`/`unless`/`begin`/block \
wrappers) rather than reproducing its exact recursive node pattern; a \
`Class.new`/`Struct.new` block body (RuboCop's `class_constructor?`) is not \
specially recognized as class-like, only as a transparent block wrapper \
(matches in practice, since that branch of RuboCop's own pattern also just \
tunnels through `any_block`).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let (min_lines, max_lines) = match options.get("NumberOfEmptyLines") {
            Some(OptionValue::List(items)) if !items.is_empty() => {
                let first = items[0].as_int().unwrap_or(1);
                let last = items[items.len() - 1].as_int().unwrap_or(first);
                (first, last)
            }
            Some(value) => {
                let n = value.as_int().unwrap_or(1);
                (n, n)
            }
            None => (1, 1),
        };
        Ok(Self {
            enabled_for: EnabledFor {
                method: options.bool("EmptyLineBetweenMethodDefs"),
                class: options.bool("EmptyLineBetweenClassDefs"),
                module: options.bool("EmptyLineBetweenModuleDefs"),
            },
            allow_adjacent_one_line_defs: options.bool("AllowAdjacentOneLineDefs"),
            def_like_macros: options
                .str_list("DefLikeMacros")
                .into_iter()
                .map(String::into_bytes)
                .collect(),
            min_lines,
            max_lines,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(stmts) = node.as_statements_node() else { return };
        let body = stmts.body();
        if body.len() < 2 {
            return;
        }
        let scope = in_macro_scope(ctx.ancestors());
        let mut prev: Option<Candidate> = None;
        for child in &body {
            let cur = self.classify(&child, ctx, scope);
            if let (Some(p), Some(c)) = (&prev, &cur) {
                self.check_pair(p, c, ctx);
            }
            prev = cur;
        }
    }
}
