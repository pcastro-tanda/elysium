//! `Style/ClassAndModuleChildren`, ported from RuboCop's
//! `lib/rubocop/cop/style/class_and_module_children.rb`, plus the `Alignment`
//! and `ConfigurableEnforcedStyle` mixins it includes.
//!
//! Single traversal, subscribed to `StatementsNode`/`ClassNode`/`ModuleNode`:
//! entering a `StatementsNode` records two facts about its direct items,
//! needed later purely from spans (no re-walking):
//! - [`ClassAndModuleChildren::sole_child_of_class_or_module`]: a lone item
//!   whose `StatementsNode` is itself the body of a `ClassNode`/`ModuleNode`
//!   -- RuboCop's `node.parent&.type?(:class, :module)`, accounting for
//!   whitequark's single-statement unwrapping (Prism's `StatementsNode`
//!   always wraps, even a single statement).
//! - [`ClassAndModuleChildren::left_sibling`]: the immediately preceding
//!   item's span, for every item but the first -- RuboCop's
//!   `Node#left_sibling`.
//!
//! Entering a `ClassNode` records its own span and constant-path text into
//! [`ClassAndModuleChildren::class_defs`] before checking it, so a later
//! sibling's `replace_namespace_keyword` search (`left_sibling.each_node(
//! :class).find { |c| c.identifier == namespace }`) is answered as a span
//! containment + text-equality scan over everything already visited --
//! guaranteed complete because depth-first pre-order fully visits a left
//! sibling's subtree before this node is ever entered.

use std::collections::{HashMap, HashSet};

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, OptionValue, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{ClassNode, ModuleNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `NESTED_MSG`.
const NESTED_MSG: &str = "Use nested module/class definitions instead of compact style.";
/// RuboCop's `COMPACT_MSG`.
const COMPACT_MSG: &str = "Use compact module/class definition instead of nested style.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Nested,
    Compact,
}

impl Style {
    fn parse(s: &str) -> Self {
        if s == "compact" {
            Style::Compact
        } else {
            Style::Nested
        }
    }
}

/// Checks that namespaced classes and modules are defined with a consistent
/// style.
#[derive(Debug, Clone)]
pub struct ClassAndModuleChildren {
    /// The raw `EnforcedStyle` option (RuboCop's `style`).
    style: Style,
    /// RuboCop's `style_for_classes`.
    style_for_classes: Style,
    /// RuboCop's `style_for_modules`.
    style_for_modules: Style,
    /// RuboCop's `configured_indentation_width`.
    indentation_width: i64,
    /// See the module doc comment.
    left_sibling: HashMap<u32, Span>,
    /// See the module doc comment.
    sole_child_of_class_or_module: HashSet<u32>,
    /// See the module doc comment.
    class_defs: Vec<(Span, Box<str>)>,
}

impl ClassAndModuleChildren {
    /// RuboCop's `on_class`/`on_module` plus the shared `check_style`.
    fn check_class(&mut self, class: &ClassNode<'_>, node: &Node<'_>, ctx: &mut Context<'_>) {
        // RuboCop's `on_class`: `return if node.parent_class && style != :nested`.
        if class.superclass().is_some() && self.style != Style::Nested {
            return;
        }
        let constant_path = class.constant_path();
        let body = class.body();
        self.check_style(node, &constant_path, body, self.style_for_classes, ctx);
    }

    fn check_module(&mut self, module: &ModuleNode<'_>, node: &Node<'_>, ctx: &mut Context<'_>) {
        let constant_path = module.constant_path();
        let body = module.body();
        self.check_style(node, &constant_path, body, self.style_for_modules, ctx);
    }

    /// RuboCop's `check_style`.
    fn check_style(
        &mut self,
        node: &Node<'_>,
        constant_path: &Node<'_>,
        body: Option<Node<'_>>,
        style: Style,
        ctx: &mut Context<'_>,
    ) {
        // RuboCop's `return if node.identifier.namespace&.cbase_type?`.
        if is_absolute_path(constant_path) {
            return;
        }
        match style {
            Style::Nested => self.check_nested_style(node, constant_path, ctx),
            Style::Compact => self.check_compact_style(node, constant_path, body, ctx),
        }
    }

    /// RuboCop's `check_nested_style`.
    fn check_nested_style(&self, node: &Node<'_>, constant_path: &Node<'_>, ctx: &mut Context<'_>) {
        if !is_compact_path(constant_path) {
            return;
        }
        if self.sole_child_of_class_or_module.contains(&node.span().start) {
            return;
        }
        let span = constant_path.span();
        // RuboCop's `autocorrect`: `return if node.class_type? && node.parent_class
        // && style != :nested` -- unreachable in practice (`check_class`'s own
        // gate above already guarantees `style == Nested` whenever there is a
        // superclass), ported anyway for fidelity.
        if self.skip_fix_for_superclass(node) {
            ctx.report(&Self::META, span, NESTED_MSG);
            return;
        }
        let fix = self.build_nest_fix(node, constant_path, ctx);
        ctx.report_with_fix(&Self::META, span, NESTED_MSG, fix);
    }

    /// RuboCop's `check_compact_style`.
    fn check_compact_style(
        &self,
        node: &Node<'_>,
        constant_path: &Node<'_>,
        body: Option<Node<'_>>,
        ctx: &mut Context<'_>,
    ) {
        if self.sole_child_of_class_or_module.contains(&node.span().start) {
            return;
        }
        let Some(inner) = single_class_or_module_child(body) else { return };
        let span = constant_path.span();
        if self.skip_fix_for_superclass(node) {
            ctx.report(&Self::META, span, COMPACT_MSG);
            return;
        }
        let fix = self.build_compact_fix(node, constant_path, &inner, ctx);
        ctx.report_with_fix(&Self::META, span, COMPACT_MSG, fix);
    }

    /// RuboCop's `autocorrect`'s guard: `node.class_type? && node.parent_class
    /// && style != :nested`.
    fn skip_fix_for_superclass(&self, node: &Node<'_>) -> bool {
        node.as_class_node().is_some_and(|c| c.superclass().is_some())
            && self.style != Style::Nested
    }

    /// RuboCop's `nest_definition`.
    fn build_nest_fix(&self, node: &Node<'_>, constant_path: &Node<'_>, ctx: &Context<'_>) -> Fix {
        let path = constant_path.as_constant_path_node().expect("compact path checked by caller");
        let namespace = path.parent().expect("compact path has a namespace parent");
        let delimiter = path.delimiter_loc().span();

        let node_span = node.span();
        let keyword = keyword_span(node);
        let end = end_keyword_span(node);

        let own_col = ctx.line_col(node_span.start).column;
        let width = u32::try_from(self.indentation_width.max(0)).unwrap_or(2);
        let own_leading = leading_whitespace(ctx, node_span.start);
        let padding = format!("{}{own_leading}", " ".repeat((own_col + width) as usize));
        let end_col = ctx.line_col(end.start).column;
        let padding_for_trailing_end = sub_first(&padding, &" ".repeat(end_col as usize));

        let namespace_text = String::from_utf8_lossy(ctx.text(namespace.span())).into_owned();
        let namespace_keyword = self.namespace_keyword_for(node_span.start, &namespace_text);
        let original_keyword = String::from_utf8_lossy(ctx.text(keyword)).into_owned();

        let edits = vec![
            Edit::replace(keyword, namespace_keyword.as_bytes().to_vec()),
            Edit::replace(delimiter, format!("\n{padding}{original_keyword} ").into_bytes()),
            Edit::replace(
                end,
                format!("{padding_for_trailing_end}end\n{own_leading}end").into_bytes(),
            ),
        ];
        Fix { applicability: Applicability::Unsafe, edits }
    }

    /// RuboCop's `replace_namespace_keyword`'s `class_definition` search:
    /// the immediate left sibling's subtree for a `class` node whose own
    /// identifier equals `namespace_text`.
    fn namespace_keyword_for(&self, node_start: u32, namespace_text: &str) -> &'static str {
        let Some(sibling) = self.left_sibling.get(&node_start).copied() else { return "module" };
        let found = self.class_defs.iter().any(|(span, text)| {
            span.start >= sibling.start
                && span.end <= sibling.end
                && text.as_ref() == namespace_text
        });
        if found {
            "class"
        } else {
            "module"
        }
    }

    /// RuboCop's `compact_definition`.
    fn build_compact_fix(
        &self,
        node: &Node<'_>,
        constant_path: &Node<'_>,
        inner: &Node<'_>,
        ctx: &Context<'_>,
    ) -> Fix {
        let inner_path = match inner {
            Node::ClassNode { .. } => inner.as_class_node().expect("kind matched").constant_path(),
            Node::ModuleNode { .. } => {
                inner.as_module_node().expect("kind matched").constant_path()
            }
            _ => unreachable!("single_class_or_module_child guarantees class/module"),
        };
        let inner_word = keyword_word(inner);
        let inner_end = end_keyword_span(inner);

        let keyword = keyword_span(node);
        let replace_range = Span::new(keyword.start, inner_path.span().end);

        // RuboCop's `compact_replacement`: the inner node's own leading
        // comments (physically inside `replace_range`), hoisted above the
        // compacted declaration line, then the declaration itself.
        let mut lines: Vec<String> = ctx
            .comments()
            .iter()
            .filter(|c| c.span.start >= replace_range.start && c.span.end <= replace_range.end)
            .map(|c| String::from_utf8_lossy(ctx.text(c.span)).into_owned())
            .collect();
        lines.push(format!(
            "{inner_word} {}::{}",
            String::from_utf8_lossy(ctx.text(constant_path.span())),
            String::from_utf8_lossy(ctx.text(inner_path.span())),
        ));
        let replacement = lines.join("\n");

        let mut edits = vec![Edit::replace(replace_range, replacement.into_bytes())];

        // RuboCop's `remove_end`.
        let inner_name_line = ctx.line_col(inner_path.span().start).line;
        let inner_end_line = ctx.line_col(inner_end.start).line;
        let remove_begin = if inner_name_line == inner_end_line {
            inner_path.span().end
        } else {
            let indent_len = u32::try_from(leading_whitespace(ctx, inner.span().start).len())
                .unwrap_or(u32::MAX);
            inner_end.start.saturating_sub(indent_len)
        };
        let file_len = u32::try_from(ctx.source().bytes().len()).unwrap_or(u32::MAX);
        let semicolon = remove_begin < file_len
            && ctx.text(Span::new(remove_begin, remove_begin + 1)).first() == Some(&b';');
        let adjustment = u32::from(!semicolon);
        let remove_end = (inner_end.end + adjustment).min(file_len);
        let remove_range = Span::new(remove_begin, remove_end);
        edits.push(Edit::delete(remove_range));

        // RuboCop's `unindent`.
        if let Some(delta) = self.unindent_delta(node, inner, ctx) {
            let taboo = [replace_range, remove_range];
            edits.extend(build_alignment_edits(ctx, node.span(), delta, &taboo));
        }

        Fix { applicability: Applicability::Unsafe, edits }
    }

    /// RuboCop's `unindent`'s `column_delta` computation (the actual shift
    /// is delegated to [`build_alignment_edits`]); `None` for its early
    /// returns (`node.body.children.last` absent, or already aligned).
    fn unindent_delta(&self, node: &Node<'_>, inner: &Node<'_>, ctx: &Context<'_>) -> Option<i64> {
        let inner_body = match inner {
            Node::ClassNode { .. } => inner.as_class_node().expect("kind matched").body(),
            Node::ModuleNode { .. } => inner.as_module_node().expect("kind matched").body(),
            _ => unreachable!("single_class_or_module_child guarantees class/module"),
        };
        let reference = last_child_reference(inner_body)?;
        let last_indent = leading_whitespace(ctx, reference.span().start);
        let node_indent = leading_whitespace(ctx, node.span().start);
        let last_cols = columns_of(&last_indent, self.indentation_width);
        let node_cols = columns_of(&node_indent, self.indentation_width);
        if node_cols == last_cols {
            return None;
        }
        let delta = self.indentation_width - last_cols;
        if delta == 0 {
            None
        } else {
            Some(delta)
        }
    }
}

impl Rule for ClassAndModuleChildren {
    const META: RuleMeta = RuleMeta {
        name: "Style/ClassAndModuleChildren",
        department: Department::Style,
        summary: "Checks that namespaced classes and modules are defined with a consistent style.",
        explanation: "\
With `nested` style, classes and modules should be defined separately (one
constant on each line, without `::`). With `compact` style, classes and
modules should be defined with fully qualified names (using `::` for
namespaces). The compact style is only forced for classes/modules with one
child.

By default `EnforcedStyle` applies to both classes and modules; separate
styles can be set with `EnforcedStyleForClasses`/`EnforcedStyleForModules`.

```ruby
# EnforcedStyle: nested (default)
# good
class Foo
  class Bar
  end
end

# EnforcedStyle: compact
# good
class Foo::Bar
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::StatementsNode, NodeKind::ClassNode, NodeKind::ModuleNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("nested"),
                allowed: &["nested", "compact"],
                doc: "Whether children should be nested (one per line) or compacted (`A::B`).",
            },
            ConfigOption {
                name: "EnforcedStyleForClasses",
                default: ConfigDefault::Nil,
                allowed: &["nested", "compact"],
                doc: "Overrides `EnforcedStyle` for classes only, when set.",
            },
            ConfigOption {
                name: "EnforcedStyleForModules",
                default: ConfigDefault::Nil,
                allowed: &["nested", "compact"],
                doc: "Overrides `EnforcedStyle` for modules only, when set.",
            },
        ],
        blind_spots: "\
- `AlignmentCorrector`'s heredoc/delimited-string-literal protection
  (`inside_string_ranges`) and `=begin`/`=end` block-comment guard
  (`block_comment_within?`) are not ported: an unindent shift touching a
  heredoc body or straddling a block comment could mis-edit it. No fixture
  exercises this.
- `Layout/IndentationStyle`'s own `IndentationWidth` (tab-to-column weight)
  is not read as a separate peer; tabs are weighted using this cop's own
  `configured_indentation_width`, which matches upstream's fallback chain
  whenever `Layout/IndentationStyle: IndentationWidth` is unset (the common
  case).
- Compacting (or splitting) a name with two or more `::` levels only
  resolves one level per lint pass, matching upstream's own single-node
  `add_offense` (only the outermost namespace is flagged per pass); the
  fixture harness's iterate-to-fixpoint autocorrect loop resolves the rest.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = Style::parse(options.style("EnforcedStyle")?);
        let style_for_classes = override_style(options, "EnforcedStyleForClasses").unwrap_or(style);
        let style_for_modules = override_style(options, "EnforcedStyleForModules").unwrap_or(style);
        let indentation_width = match options.peer("Layout/IndentationWidth", "Width") {
            Some(value) => value.as_int().unwrap_or(2),
            None => 2,
        };
        Ok(Self {
            style,
            style_for_classes,
            style_for_modules,
            indentation_width,
            left_sibling: HashMap::new(),
            sole_child_of_class_or_module: HashSet::new(),
            class_defs: Vec::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.left_sibling.clear();
        self.sole_child_of_class_or_module.clear();
        self.class_defs.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::StatementsNode => self.record_statements(node, ctx),
            NodeKind::ClassNode => {
                let class = node.as_class_node().expect("kind matched");
                self.record_class_def(&class, ctx);
                self.check_class(&class, node, ctx);
            }
            NodeKind::ModuleNode => {
                let module = node.as_module_node().expect("kind matched");
                self.check_module(&module, node, ctx);
            }
            _ => {}
        }
    }
}

impl ClassAndModuleChildren {
    /// Records, for this `StatementsNode`'s direct items, the facts the
    /// module doc comment describes.
    fn record_statements(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let stmts = node.as_statements_node().expect("kind matched");
        let items: Vec<Node<'_>> = stmts.body().iter().collect();
        if items.len() == 1 {
            if let Some(parent) = ctx.parent() {
                if matches!(parent.kind, NodeKind::ClassNode | NodeKind::ModuleNode) {
                    self.sole_child_of_class_or_module.insert(items[0].span().start);
                }
            }
        }
        for i in 1..items.len() {
            self.left_sibling.insert(items[i].span().start, items[i - 1].span());
        }
    }

    fn record_class_def(&mut self, class: &ClassNode<'_>, ctx: &Context<'_>) {
        let span = class.location().span();
        let path = class.constant_path();
        let text = String::from_utf8_lossy(ctx.text(path.span())).into_owned();
        self.class_defs.push((span, text.into_boxed_str()));
    }
}

/// RuboCop's `cop_config['EnforcedStyleForClasses'] || style`-style raw
/// (unvalidated) hash read, `None` when unset or not one of the two styles.
fn override_style(options: &RuleOptions, key: &str) -> Option<Style> {
    match options.get(key) {
        Some(OptionValue::Str(s)) if s == "nested" => Some(Style::Nested),
        Some(OptionValue::Str(s)) if s == "compact" => Some(Style::Compact),
        _ => None,
    }
}

/// RuboCop's `node.identifier.namespace&.cbase_type?`: a top-level `::Foo`
/// path (a `ConstantPathNode` with no `parent`).
fn is_absolute_path(path: &Node<'_>) -> bool {
    path.as_constant_path_node().is_some_and(|p| p.parent().is_none())
}

/// RuboCop's `compact_node_name?`: the constant path has a real (non-cbase)
/// namespace prefix. Only meaningful once [`is_absolute_path`] excluded the
/// bare-`::` form.
fn is_compact_path(path: &Node<'_>) -> bool {
    path.as_constant_path_node().is_some()
}

/// RuboCop's `needs_compacting?`, normalizing Prism's always-wrapping
/// `StatementsNode` to whitequark's single-statement unwrap: `body` passes
/// only when it holds exactly one statement and that statement is itself a
/// `class`/`module` definition.
fn single_class_or_module_child(body: Option<Node<'_>>) -> Option<Node<'_>> {
    let body = body?;
    let stmts = body.as_statements_node()?;
    let mut items = stmts.body().iter();
    let first = items.next()?;
    if items.next().is_some() {
        return None;
    }
    matches!(first.kind(), NodeKind::ClassNode | NodeKind::ModuleNode).then_some(first)
}

/// RuboCop's `node.body.children.last`: for a `class`/`module` AST node,
/// `children.last` is always its own body slot (whitequark unwraps a single
/// statement there, or leaves the synthetic `begin` node for several) --
/// only ever used here for `leading_spaces`, i.e. the leading whitespace of
/// its own first line, which for either shape is the first statement's own
/// line.
fn last_child_reference(body: Option<Node<'_>>) -> Option<Node<'_>> {
    let body = body?;
    let stmts = body.as_statements_node()?;
    stmts.body().iter().next()
}

/// RuboCop's `node.loc.keyword` (`class_keyword_loc`/`module_keyword_loc`).
fn keyword_span(node: &Node<'_>) -> Span {
    match node {
        Node::ClassNode { .. } => {
            node.as_class_node().expect("kind matched").class_keyword_loc().span()
        }
        Node::ModuleNode { .. } => {
            node.as_module_node().expect("kind matched").module_keyword_loc().span()
        }
        _ => unreachable!("class/module node expected"),
    }
}

/// RuboCop's `node.loc.end`.
fn end_keyword_span(node: &Node<'_>) -> Span {
    match node {
        Node::ClassNode { .. } => {
            node.as_class_node().expect("kind matched").end_keyword_loc().span()
        }
        Node::ModuleNode { .. } => {
            node.as_module_node().expect("kind matched").end_keyword_loc().span()
        }
        _ => unreachable!("class/module node expected"),
    }
}

/// RuboCop's `node.body.type` (used as the compacted declaration's keyword).
fn keyword_word(node: &Node<'_>) -> &'static str {
    match node {
        Node::ClassNode { .. } => "class",
        Node::ModuleNode { .. } => "module",
        _ => unreachable!("class/module node expected"),
    }
}

/// RuboCop's `leading_spaces`: the run of `[ \t]` at the start of the line
/// containing `offset`.
fn leading_whitespace(ctx: &Context<'_>, offset: u32) -> String {
    let line = ctx.line_col(offset).line;
    let text = ctx.line_text(line);
    let end = text.iter().take_while(|&&b| b == b' ' || b == b'\t').count();
    String::from_utf8_lossy(&text[..end]).into_owned()
}

/// RuboCop's `spaces_size`: each byte counts as one column, except a tab,
/// which counts as `tab_width` (this cop's own `tab_indentation_width`,
/// approximated here as `configured_indentation_width`; see `blind_spots`).
fn columns_of(s: &str, tab_width: i64) -> i64 {
    s.bytes().map(|b| if b == b'\t' { tab_width } else { 1 }).sum()
}

/// Ruby's `String#sub(needle, '')`: removes the first occurrence of
/// `needle`, or returns `haystack` unchanged (including when `needle` is
/// empty, matching Ruby's zero-width-match-replaced-by-nothing semantics).
fn sub_first(haystack: &str, needle: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    match haystack.find(needle) {
        Some(idx) => {
            let mut out = String::with_capacity(haystack.len() - needle.len());
            out.push_str(&haystack[..idx]);
            out.push_str(&haystack[idx + needle.len()..]);
            out
        }
        None => haystack.to_string(),
    }
}

/// RuboCop's `AlignmentCorrector.correct`: one edit per physical line of
/// `node_span`'s *original* source, per `line_edit`, skipping any edit whose
/// range falls inside `taboo` (already covered by another edit in this same
/// `Fix` -- upstream's `Corrector` silently swallows such nested edits;
/// `Fix::edits` must be non-overlapping here, so they are built pre-filtered
/// instead).
fn build_alignment_edits(
    ctx: &Context<'_>,
    node_span: Span,
    delta: i64,
    taboo: &[Span],
) -> Vec<Edit> {
    let bytes = ctx.source().bytes();
    let mut edits = Vec::new();
    let mut pos = node_span.start;
    loop {
        if let Some(edit) = line_edit(bytes, pos, delta) {
            let overlaps_taboo =
                taboo.iter().any(|t| edit.span.start < t.end && t.start < edit.span.end);
            if !overlaps_taboo {
                edits.push(edit);
            }
        }
        let end = node_span.end.min(u32::try_from(bytes.len()).unwrap_or(u32::MAX));
        match bytes[pos as usize..end as usize].iter().position(|&b| b == b'\n') {
            Some(i) => pos += u32::try_from(i).unwrap_or(u32::MAX) + 1,
            None => break,
        }
        if pos >= end {
            break;
        }
    }
    edits
}

/// RuboCop's `calculate_range` + `autocorrect_line`, combined and resolved
/// to a single optional edit for the line starting at `line_start`.
fn line_edit(bytes: &[u8], line_start: u32, delta: i64) -> Option<Edit> {
    if delta > 0 {
        // RuboCop's `range.resize(1).source != "\n"`: never indent a blank line.
        if bytes.get(line_start as usize) == Some(&b'\n') {
            return None;
        }
        let width = usize::try_from(delta).unwrap_or(0);
        return Some(Edit::insert(line_start, " ".repeat(width).into_bytes()));
    }
    let abs = u32::try_from(-delta).unwrap_or(u32::MAX);
    let starts_with_space = bytes.get(line_start as usize) == Some(&b' ');
    let (start, end) = if starts_with_space {
        (line_start, line_start + abs)
    } else {
        if line_start < abs {
            return None;
        }
        (line_start - abs, line_start)
    };
    if end as usize > bytes.len() || start > end {
        return None;
    }
    let slice = &bytes[start as usize..end as usize];
    if !slice.is_empty() && slice.iter().all(|&b| b == b' ' || b == b'\t') {
        Some(Edit::delete(Span::new(start, end)))
    } else {
        None
    }
}
