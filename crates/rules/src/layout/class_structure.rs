//! `Layout/ClassStructure`, ported from RuboCop's
//! `lib/rubocop/cop/layout/class_structure.rb` plus the `VisibilityHelp`
//! (`lib/rubocop/cop/mixin/visibility_help.rb`) and `CommentsHelp`
//! (`lib/rubocop/cop/mixin/comments_help.rb`) mixins it includes -- though
//! this cop overrides `CommentsHelp`'s `begin_pos_with_comment`/
//! `end_position_for`/`start_line_position` with its own (heredoc-aware,
//! trailing-comment-tolerant) versions, which is what is ported below.
//!
//! Upstream hooks `on_class`/`on_sclass` (never `on_module`) and walks only
//! the *direct* elements of that one class/singleton-class body -- a
//! `begin`/`kwbegin` wrapper with no `rescue`/`ensure`/`else` is transparent
//! (its statements are hoisted up one level), everything else (most
//! importantly a nested `class`/`module`/`class << self`) is one opaque
//! element, never recursed into.
//!
//! Prism always wraps a class/sclass body in a [`NodeKind::StatementsNode`]
//! (even a single-statement one, unlike whitequark, which elides it), so
//! `class_elements` here starts from that directly instead of whitequark's
//! `class_def.type?(:begin, :kwbegin)` check.
//!
//! Sibling adjacency (`left_siblings`/`right_siblings` in upstream, used by
//! `node_visibility`, `movable_span`, `movable_group` and `barrier?`) is
//! computed here from this cop's own flattened element list rather than
//! real Prism parent/child relationships -- see `blind_spots`.
//!
//! `node_visibility_from_visibility_inline_on_def` (upstream: `parent.method_name if
//! visibility_inline_on_def?(parent)`) is not ported: it only ever matches a `def`/`defs` whose
//! *parent* is a bare `private def foo`-style wrapping call, but such a wrapped def is never
//! itself a class element here -- the wrapping `send` is (Prism attaches a call's block/argument
//! inline; whitequark's separate wrapping node is what upstream's `parent` walks up to), and that
//! wrapping `send` is classified directly by `find_send_node_category` without ever asking
//! `node_visibility` of the `def` it wraps.

use std::ops::Range;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::is_recursive_basic_literal;
use ruby_ast::{each_descendant, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// `VisibilityHelp::VISIBILITY_SCOPES`.
const VISIBILITY_SCOPES: [&str; 3] = ["public", "protected", "private"];

/// Upstream `MSG`.
fn message(category: &str, previous: &str) -> String {
    format!("`{category}` is supposed to appear before `{previous}`.")
}

/// Checks if the code style follows the `ExpectedOrder` configuration.
#[derive(Debug, Clone)]
pub struct ClassStructure {
    expected_order: Vec<String>,
    /// `cop_config['Categories']`, in declaration order (lookup is a linear
    /// scan either way, matching upstream's `Hash#find`).
    categories: Vec<(String, Vec<String>)>,
}

impl Rule for ClassStructure {
    const META: RuleMeta = RuleMeta {
        name: "Layout/ClassStructure",
        department: Department::Layout,
        summary: "Checks if the code style follows the `ExpectedOrder` configuration.",
        explanation: "`Categories` allows mapping macro names into a category. Consider an \
            example of code style that covers the following order: module inclusion \
            (`include`, `prepend`, `extend`), constants, associations, public attribute macros, \
            other macros, public class methods, the initializer, public instance methods, \
            protected attribute macros and methods, then private attribute macros and methods. \
            Simply enabling the cop with `Enabled: true` does not use that example order --\
            `ExpectedOrder` and `Categories` must both be configured for macro ordering (e.g. \
            `attr_reader`) to be enforced.\n\n\
            Autocorrection is unsafe because class methods and module inclusion can behave \
            differently based on which methods or constants have already been defined; \
            constants are only moved when assigned a literal.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode, NodeKind::SingletonClassNode],
        config: &[
            ConfigOption {
                name: "ExpectedOrder",
                default: ConfigDefault::StrList(&[
                    "module_inclusion",
                    "constants",
                    "public_class_methods",
                    "initializer",
                    "public_methods",
                    "protected_methods",
                    "private_class_methods",
                    "private_methods",
                ]),
                allowed: &[],
                doc: "The order classes and modules should be structured in.",
            },
            ConfigOption {
                name: "Categories",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "A hash mapping a category name to the list of method names grouped under \
                    it, for `ExpectedOrder` purposes. Default: `{\"module_inclusion\": \
                    [\"include\", \"prepend\", \"extend\"]}`.",
            },
        ],
        blind_spots: "Sibling adjacency for the autocorrect anchor/barrier/movable-group search \
            is derived from this cop's own flattened direct-element list rather than real Prism \
            parent/child relationships, so a misordered element nested inside an explicit \
            `begin...end` grouping block can never be reordered across that block's boundary \
            (no fixture exercises this either way).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let expected_order = options.str_list("ExpectedOrder");
        let categories = match options.get("Categories") {
            Some(OptionValue::Map(entries)) => {
                entries.iter().map(|(name, value)| (name.clone(), value.to_string_list())).collect()
            }
            _ => vec![(
                "module_inclusion".to_string(),
                vec!["include".to_string(), "prepend".to_string(), "extend".to_string()],
            )],
        };
        Ok(Self { expected_order, categories })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let body = match node.kind() {
            NodeKind::ClassNode => node.as_class_node().and_then(|n| n.body()),
            NodeKind::SingletonClassNode => node.as_singleton_class_node().and_then(|n| n.body()),
            _ => return,
        };
        let Some(body) = body else { return };
        let Some(stmts) = body.as_statements_node() else { return };

        let mut elements: Vec<Node<'_>> = Vec::new();
        flatten_class_elements(&stmts, &mut elements);
        if elements.is_empty() {
            return;
        }
        let classifications: Vec<String> =
            elements.iter().enumerate().map(|(i, n)| self.classify(n, &elements, i)).collect();
        let ignored: Vec<bool> =
            (0..elements.len()).map(|i| self.is_ignored(&elements, &classifications, i)).collect();

        // Upstream's `out_of_order_elements`: every element whose category
        // sorts earlier than the highest-priority category seen so far.
        let mut offending: Vec<(usize, String)> = Vec::new();
        let mut max_index: Option<usize> = None;
        let mut previous_category: Option<&str> = None;
        for i in 0..elements.len() {
            if ignored[i] {
                continue;
            }
            let idx = self
                .expected_order
                .iter()
                .position(|c| c == &classifications[i])
                .expect("not ignored: category is in expected_order");
            if max_index.is_some_and(|max| idx < max)
                && Some(classifications[i].as_str()) != previous_category
            {
                let previous = self.expected_order[max_index.expect("checked above")].clone();
                offending.push((i, message(&classifications[i], &previous)));
            }
            if max_index.is_none_or(|max| idx > max) {
                max_index = Some(idx);
            }
            previous_category = Some(classifications[i].as_str());
        }
        if offending.is_empty() {
            return;
        }

        // Two offending elements can resolve to the identical insertion
        // anchor (e.g. two out-of-order elements both belonging before the
        // same later sibling). Upstream's real `Parser::TreeRewriter`
        // tolerates any number of independent `insert_before` calls at the
        // same position; this engine's `Fix`/`Edit` model does not -- two
        // diagnostics each inserting at the same offset conflict and only
        // the first-processed one survives a single fix pass. So every
        // offending element sharing an anchor is corrected by a single
        // combined `Fix` attached to the first (lowest source position) of
        // them; the rest are reported with no fix of their own (their
        // correction already happened as part of the first one's).
        let anchor_of: Vec<Option<usize>> = offending
            .iter()
            .map(|&(i, _)| {
                if is_dynamic_constant(&elements[i]) {
                    None
                } else {
                    self.insertion_anchor(&elements, &classifications, &ignored, i)
                }
            })
            .collect();
        let mut carrier_of_anchor: Vec<(usize, usize)> = Vec::new();
        for (pos, anchor) in anchor_of.iter().enumerate() {
            if let Some(anchor) = anchor {
                if !carrier_of_anchor.iter().any(|&(a, _)| a == *anchor) {
                    carrier_of_anchor.push((*anchor, pos));
                }
            }
        }

        for (pos, (i, msg)) in offending.iter().enumerate() {
            let span = elements[*i].span();
            let is_carrier = anchor_of[pos].is_some_and(|anchor| {
                carrier_of_anchor.iter().any(|&(a, p)| a == anchor && p == pos)
            });
            let fix = if is_carrier {
                #[allow(clippy::unnecessary_unwrap)]
                let anchor = anchor_of[pos].unwrap();
                let anchor_begin = begin_pos_with_comment(ctx, &elements[anchor]);
                let mut edits = Vec::new();
                for (pos2, (i2, _)) in offending.iter().enumerate() {
                    if anchor_of[pos2] != Some(anchor) {
                        continue;
                    }
                    let group = movable_group(&elements, &classifications, &ignored, *i2);
                    for j in group {
                        let begin = begin_pos_with_comment(ctx, &elements[j]);
                        let end = end_position_for(ctx, &elements[j]);
                        let edit_span = Span::new(begin, end);
                        edits.push(Edit::insert(anchor_begin, ctx.text(edit_span).to_vec()));
                        edits.push(Edit::delete(edit_span));
                    }
                }
                Some(Fix { applicability: Applicability::Unsafe, edits })
            } else {
                None
            };
            match fix {
                Some(fix) => ctx.report_with_fix(&Self::META, span, msg.clone(), fix),
                None => ctx.report(&Self::META, span, msg.clone()),
            }
        }
    }
}

impl ClassStructure {
    /// Upstream's `Hash#find` over `Categories`: the category name owning a
    /// bucket that lists `name`.
    fn find_category(&self, name: &str) -> Option<String> {
        self.categories
            .iter()
            .find(|(_, names)| names.iter().any(|n| n == name))
            .map(|(category, _)| category.clone())
    }

    /// Upstream's `classify`: a node's category string (before `ignore?`
    /// decides whether it counts).
    fn classify(&self, node: &Node<'_>, elements: &[Node<'_>], i: usize) -> String {
        match node.kind() {
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                self.find_send_node_category(&call, elements, i)
            }
            NodeKind::DefNode => {
                let def = node.as_def_node().expect("kind matched");
                let name = if def.receiver().is_none() {
                    if def.name().as_slice() == b"initialize" {
                        "initializer".to_string()
                    } else {
                        format!("{}_methods", node_visibility(elements, i))
                    }
                } else {
                    "public_class_methods".to_string()
                };
                self.find_category(&name).unwrap_or(name)
            }
            NodeKind::ConstantWriteNode | NodeKind::ConstantPathWriteNode => {
                self.find_category("constants").unwrap_or_else(|| "constants".to_string())
            }
            NodeKind::SingletonClassNode => self
                .find_category("class_singleton")
                .unwrap_or_else(|| "class_singleton".to_string()),
            other => {
                let name = format!("{other:?}");
                self.find_category(&name).unwrap_or(name)
            }
        }
    }

    /// Upstream's `find_send_node_category`.
    fn find_send_node_category(
        &self,
        call: &ruby_ast::node::CallNode<'_>,
        elements: &[Node<'_>],
        i: usize,
    ) -> String {
        let name = String::from_utf8_lossy(call.name().as_slice()).into_owned();
        let key = self.find_category(&name).unwrap_or_else(|| name.clone());
        let visibility_key = if is_def_modifier(call) {
            if key.ends_with("_class_method") {
                format!("{key}s")
            } else {
                format!("{key}_methods")
            }
        } else {
            format!("{}_{key}", node_visibility(elements, i))
        };
        if self.expected_order.contains(&visibility_key) {
            visibility_key
        } else {
            key
        }
    }

    /// Upstream's `ignore?`.
    fn is_ignored(&self, elements: &[Node<'_>], classifications: &[String], i: usize) -> bool {
        let classification = &classifications[i];
        classification.ends_with('=')
            || !self.expected_order.contains(classification)
            || is_private_constant(&elements[i], elements)
    }

    /// Upstream's `visibility_dependent?`.
    fn visibility_dependent(&self, elements: &[Node<'_>], i: usize) -> bool {
        if let Some(def) = elements[i].as_def_node() {
            return def.receiver().is_none();
        }
        let Some(call) = elements[i].as_call_node() else { return false };
        if is_def_modifier(&call) {
            return false;
        }
        let name = String::from_utf8_lossy(call.name().as_slice()).into_owned();
        let key = self.find_category(&name).unwrap_or(name);
        VISIBILITY_SCOPES.iter().any(|v| self.expected_order.contains(&format!("{v}_{key}")))
    }

    /// Upstream's `barrier?`.
    fn is_barrier(&self, elements: &[Node<'_>], node_i: usize, sibling_j: usize) -> bool {
        is_dynamic_constant(&elements[sibling_j])
            || (self.visibility_dependent(elements, node_i)
                && is_visibility_block(&elements[sibling_j]))
    }

    /// Upstream's `movable_span`: the left siblings `node_i` may be
    /// reordered with -- those strictly after the rightmost barrier.
    fn movable_span(&self, elements: &[Node<'_>], node_i: usize) -> Range<usize> {
        let barrier = (0..node_i).rev().find(|&j| self.is_barrier(elements, node_i, j));
        match barrier {
            Some(b) => (b + 1)..node_i,
            None => 0..node_i,
        }
    }

    /// Upstream's `insertion_anchor`.
    fn insertion_anchor(
        &self,
        elements: &[Node<'_>],
        classifications: &[String],
        ignored: &[bool],
        node_i: usize,
    ) -> Option<usize> {
        let idx = self.expected_order.iter().position(|c| c == &classifications[node_i])?;
        self.movable_span(elements, node_i).find(|&j| {
            !ignored[j]
                && self
                    .expected_order
                    .iter()
                    .position(|c| c == &classifications[j])
                    .is_some_and(|other| other > idx)
        })
    }
}

/// Upstream's `movable_group`.
fn movable_group(
    elements: &[Node<'_>],
    classifications: &[String],
    ignored: &[bool],
    node_i: usize,
) -> Range<usize> {
    let category = &classifications[node_i];
    let mut end = node_i + 1;
    while end < elements.len()
        && &classifications[end] == category
        && !ignored[end]
        && !is_dynamic_constant(&elements[end])
    {
        end += 1;
    }
    node_i..end
}

/// Upstream's `class_elements`/`flatten_class_elements`: the direct
/// elements of a class/sclass body, exploding a bare `begin`/`kwbegin`
/// wrapper (no `rescue`/`else`/`ensure`) but not recursing into anything
/// else.
fn flatten_class_elements<'pr>(
    stmts: &ruby_ast::node::StatementsNode<'pr>,
    out: &mut Vec<Node<'pr>>,
) {
    let body = stmts.body();
    for child in &body {
        if let Some(begin) = child.as_begin_node() {
            if begin.rescue_clause().is_none()
                && begin.else_clause().is_none()
                && begin.ensure_clause().is_none()
            {
                if let Some(inner) = begin.statements() {
                    flatten_class_elements(&inner, out);
                }
                continue;
            }
        }
        out.push(child);
    }
}

/// `VisibilityHelp#visibility_block?`: a receiver-less, argument-less call
/// to `private`/`protected`/`public` (deliberately narrower than
/// `ext::is_bare_access_modifier`, which also allows `module_function`).
fn is_visibility_block(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|c| {
        c.receiver().is_none()
            && c.arguments().is_none()
            && matches!(c.name().as_slice(), b"private" | b"protected" | b"public")
    })
}

/// `VisibilityHelp#visibility_inline_on_method_name?`: a receiver-less call
/// to `private`/`protected`/`public` with exactly one symbol-literal
/// argument naming `method_name`.
fn is_visibility_inline_on_method_name(
    call: &ruby_ast::node::CallNode<'_>,
    method_name: &[u8],
) -> bool {
    if call.receiver().is_some()
        || !matches!(call.name().as_slice(), b"private" | b"protected" | b"public")
    {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let args = args.arguments();
    args.len() == 1
        && args
            .first()
            .and_then(|a| a.as_symbol_node())
            .is_some_and(|sym| sym.unescaped() == method_name)
}

/// `VisibilityHelp#node_visibility`.
fn node_visibility(elements: &[Node<'_>], i: usize) -> &'static str {
    if let Some(def) = elements[i].as_def_node() {
        if def.receiver().is_none() {
            let name = def.name();
            let name = name.as_slice();
            let inline = elements[i + 1..].iter().rev().find_map(|n| {
                n.as_call_node().filter(|c| is_visibility_inline_on_method_name(c, name)).map(|c| {
                    match c.name().as_slice() {
                        b"private" => "private",
                        b"protected" => "protected",
                        _ => "public",
                    }
                })
            });
            if let Some(v) = inline {
                return v;
            }
        }
    }
    elements[..i]
        .iter()
        .rev()
        .find_map(|n| {
            n.as_call_node().filter(|c| is_visibility_block(&c.as_node())).map(|c| {
                match c.name().as_slice() {
                    b"private" => "private",
                    b"protected" => "protected",
                    _ => "public",
                }
            })
        })
        .unwrap_or("public")
}

/// `MethodDispatchNode#def_modifier?`: the call's sole first argument is
/// itself a `def`, or recursively another such modifier call.
fn is_def_modifier(call: &ruby_ast::node::CallNode<'_>) -> bool {
    if call.receiver().is_some() {
        return false;
    }
    let Some(args) = call.arguments() else { return false };
    let Some(first) = args.arguments().first() else { return false };
    match first.kind() {
        NodeKind::DefNode => true,
        NodeKind::CallNode => first.as_call_node().is_some_and(|inner| is_def_modifier(&inner)),
        _ => false,
    }
}

/// Upstream's `dynamic_constant?`: a namespace-less constant assigned the
/// result of a method call, unless that call is `.freeze` on a receiver
/// that is itself a recursive basic literal.
fn is_dynamic_constant(node: &Node<'_>) -> bool {
    let Some(cw) = node.as_constant_write_node() else { return false };
    let Some(call) = cw.value().as_call_node() else { return false };
    if call.name().as_slice() != b"freeze" {
        return true;
    }
    match call.receiver() {
        Some(receiver) => !is_recursive_basic_literal(&receiver),
        None => true,
    }
}

/// Upstream's `private_constant?`/`marked_as_private_constant?`.
fn is_private_constant(node: &Node<'_>, elements: &[Node<'_>]) -> bool {
    let Some(cw) = node.as_constant_write_node() else { return false };
    let name = cw.name();
    let name = name.as_slice();
    elements.iter().any(|n| {
        n.as_call_node().is_some_and(|c| {
            c.receiver().is_none()
                && c.name().as_slice() == b"private_constant"
                && c.arguments().is_some_and(|args| {
                    args.arguments().iter().any(|a| symbol_or_string_matches(&a, name))
                })
        })
    })
}

/// Whether `node` is a symbol or string literal whose content is `name`.
fn symbol_or_string_matches(node: &Node<'_>, name: &[u8]) -> bool {
    if let Some(sym) = node.as_symbol_node() {
        return sym.unescaped() == name;
    }
    if let Some(s) = node.as_string_node() {
        return s.unescaped() == name;
    }
    false
}

/// Upstream's (cop-local, overriding `CommentsHelp`) `begin_pos_with_comment`
/// plus `start_line_position`: the start of `node`'s own first line, pulled
/// back to the start of a contiguous run of comment lines directly above it
/// -- but only as far as the last *whole-line* comment in that run (a
/// trailing comment on a code line stops the climb without extending the
/// range to it).
fn begin_pos_with_comment(ctx: &Context<'_>, node: &Node<'_>) -> u32 {
    let node_line = ctx.line_col(node.span().start).line;
    let mut target_line = node_line;
    for line in (1..node_line).rev() {
        if !line_has_comment(ctx, line) {
            break;
        }
        if is_whole_line_comment(ctx, line) {
            target_line = line;
        }
    }
    start_line_position(ctx, target_line)
}

/// Whether any comment (inline or whole-line) starts on `line`.
fn line_has_comment(ctx: &Context<'_>, line: u32) -> bool {
    ctx.comments().iter().any(|c| c.line == line)
}

/// `/\A\s*#/`: `line`'s only non-blank content is a comment.
fn is_whole_line_comment(ctx: &Context<'_>, line: u32) -> bool {
    let text = ctx.line_text(line);
    match text.iter().position(|&b| b != b' ' && b != b'\t') {
        Some(i) => text[i] == b'#',
        None => false,
    }
}

/// `start_line_position`: the byte just before `line` starts -- the
/// newline terminating the previous line.
fn start_line_position(ctx: &Context<'_>, line: u32) -> u32 {
    ctx.line_span(line).start.saturating_sub(1)
}

/// Upstream's (cop-local, overriding `CommentsHelp`) `end_position_for`: the
/// end of `node`'s own last line, extended through a heredoc body's closing
/// terminator when the node's value contains one. Unlike whitequark's
/// `heredoc_end` token (which stops before the trailing newline, hence
/// upstream's explicit `+ 1`), Prism's heredoc `closing_loc` already
/// includes that newline.
fn end_position_for(ctx: &Context<'_>, node: &Node<'_>) -> u32 {
    let value = node.as_constant_write_node().map(|cw| cw.value());
    if let Some(value) = value {
        if let Some(end) = find_heredoc_closing_end(&value) {
            return end;
        }
    }
    let end_line = ctx.last_line(node.span());
    ctx.line_span(end_line).end
}

/// Upstream's `find_heredoc`: the first heredoc string literal in `node`'s
/// subtree (including `node` itself), preorder.
fn find_heredoc_closing_end(node: &Node<'_>) -> Option<u32> {
    if ruby_ast::ext::is_heredoc(node) {
        return closing_loc_end(node);
    }
    let mut found = None;
    let mut visit = |n: &Node<'_>| {
        if found.is_none() && ruby_ast::ext::is_heredoc(n) {
            found = closing_loc_end(n);
        }
    };
    each_descendant(node, &mut visit);
    found
}

/// The end offset of a heredoc string's closing terminator.
fn closing_loc_end(node: &Node<'_>) -> Option<u32> {
    if let Some(s) = node.as_string_node() {
        return s.closing_loc().map(|l| l.span().end);
    }
    if let Some(s) = node.as_interpolated_string_node() {
        return s.closing_loc().map(|l| l.span().end);
    }
    if let Some(s) = node.as_x_string_node() {
        return Some(s.closing_loc().span().end);
    }
    if let Some(s) = node.as_interpolated_x_string_node() {
        return Some(s.closing_loc().span().end);
    }
    None
}
