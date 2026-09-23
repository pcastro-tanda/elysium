//! `Style/Documentation`, ported from RuboCop's `lib/rubocop/cop/style/documentation.rb`
//! (plus the `DocumentationComment` mixin it includes from
//! `lib/rubocop/cop/mixin/documentation_comment.rb`).

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `Style/CommentAnnotation` default `Keywords`, read as a peer
/// option when configured; this is the fallback when it isn't.
const DEFAULT_ANNOTATION_KEYWORDS: &[&str] =
    &["TODO", "FIXME", "OPTIMIZE", "HACK", "REVIEW", "NOTE"];

/// One enclosing `class`/`module` node, tracked while its body is walked so
/// nested nodes can build a fully qualified identifier (RuboCop's
/// `node.each_ancestor(:class, :module)`) and check `#:nodoc: all`
/// inheritance (RuboCop's `nodoc_comment?` walking `node.parent`).
///
/// Only the qualified name is kept here: the declaration line used by
/// `nodoc_applies` is instead read from `ctx.ancestors()`, keyed by
/// position among the `ClassNode`/`ModuleNode` ancestors (which is what
/// this vector's index also tracks).
type Ancestor = String;

/// Checks for missing top-level documentation of classes and modules.
#[derive(Debug, Clone)]
pub struct Documentation {
    allowed_constants: Vec<String>,
    annotation_keywords: Vec<String>,
    ancestors: Vec<Ancestor>,
}

impl Documentation {
    /// RuboCop's `constant_allowed?`.
    fn is_allowed_constant(&self, ctx: &Context<'_>, constant_path: &Node<'_>) -> bool {
        let name = short_name(ctx, constant_path);
        self.allowed_constants.contains(&name)
    }

    /// RuboCop's `identifier`: the fully qualified name of `constant_path`,
    /// prefixed by every enclosing `class`/`module` ancestor's own name.
    fn identifier(&self, ctx: &Context<'_>, constant_path: &Node<'_>) -> String {
        let mut parts: Vec<String> = self.ancestors.clone();
        parts.push(qualify(ctx, constant_path));
        replace_first(&parts.join("::"), "::::", "::")
    }

    /// RuboCop's `documentation_comment?`: at least one of the contiguous
    /// full-line comments directly above `first_line` (RuboCop's
    /// `preceding_lines`, approximated here as a straight run of full-line
    /// comments with no blank-line or code break, which is what
    /// `Parser::Source::Comment::Associator` produces for the ordinary
    /// leading-comment-block case) is not an annotation, interpreter
    /// directive, or `# rubocop:` directive comment.
    fn has_documentation_comment(&self, ctx: &Context<'_>, first_line: u32) -> bool {
        let mut line = first_line;
        let mut found_any = false;
        let mut found_non_special = false;
        while line > 1 {
            let candidate = line - 1;
            let Some(comment) = full_line_comment(ctx, candidate) else { break };
            found_any = true;
            let text = ctx.text(comment.span);
            let special = is_annotation(text, &self.annotation_keywords)
                || is_interpreter_directive(text)
                || is_rubocop_directive(ctx, comment.line);
            if !special {
                found_non_special = true;
            }
            line = candidate;
        }
        found_any && found_non_special
    }

    /// RuboCop's `check`, shared by `on_class` (only when the class has a
    /// body) and `on_module` (unconditionally).
    fn check(
        &self,
        ctx: &mut Context<'_>,
        node: &Node<'_>,
        kind_word: &str,
        constant_path: &Node<'_>,
        body: Option<&Node<'_>>,
        own_line: u32,
    ) {
        if is_namespace(body) {
            return;
        }
        if self.has_documentation_comment(ctx, own_line) {
            return;
        }
        if self.is_allowed_constant(ctx, constant_path) {
            return;
        }
        if nodoc_applies(ctx, own_line) {
            return;
        }
        if include_statement_only_body(body) {
            return;
        }

        let identifier = self.identifier(ctx, constant_path);
        let span = Span::new(node.span().start, constant_path.span().end);
        let message =
            format!("Missing top-level documentation comment for `{kind_word} {identifier}`.");
        ctx.report(&Self::META, span, message);
    }
}

impl Rule for Documentation {
    const META: RuleMeta = RuleMeta {
        name: "Style/Documentation",
        department: Department::Style,
        summary: "Checks for missing top-level documentation of classes and modules.",
        explanation: "\
Classes with no body are exempt from the check and so are namespace
modules - modules that have nothing in their bodies except classes, other
modules, constant definitions or constant visibility declarations.

The documentation requirement is annulled if the class or module has a
`#:nodoc:` comment next to it. Likewise, `#:nodoc: all` does the same for
all its children.

```ruby
# bad
class Person
  # ...
end

module Math
end

# good
# Description/Explanation of Person class
class Person
  # ...
end

# allowed
# Class without body
class Person
end

# Namespace - A namespace can be a class or a module
# Containing a class
module Namespace
  # Description/Explanation of Person class
  class Person
    # ...
  end
end

# Containing constant visibility declaration
module Namespace
  class Private
  end

  private_constant :Private
end

# Containing constant definition
module Namespace
  Public = Class.new
end

# Macro calls
module Namespace
  extend Foo
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ClassNode, NodeKind::ModuleNode],
        config: &[ConfigOption {
            name: "AllowedConstants",
            default: ConfigDefault::StrList(&[]),
            allowed: &[],
            doc: "Constant names, without namespace, exempted from the documentation check.",
        }],
        blind_spots: "\
- Comment association is approximated as the contiguous run of full-line
  `#...` comments directly above the node's declaration line, instead of
  porting RuboCop's `Parser::Source::Comment::Associator`. This matches
  every ordinary leading-comment-block case (including a blank line or a
  trailing same-line comment on the previous statement breaking the run)
  but can differ for exotic comment placements the real associator would
  steal from an earlier sibling node.
- The `compact_namespace?` / `outer_module` special case (an RDoc-style
  `#:nodoc:` attached only to the outer segment of a compact `A::B::Test`
  path, found via a `(const (const nil? _) _)` node search) is not ported.
  Only the node's own declaration line and its actual `class`/`module`
  ancestors' declaration lines are checked for `:nodoc:`/`:nodoc: all`.
- `# rubocop:push`/`# rubocop:pop` directive comments are not recognized as
  directive comments (a `ruby_directives` limitation), so a preceding
  comment using them could be wrongly treated as documentation.
- The annotation-comment matcher approximates Ruby's
  `/^(# ?)(\\b#{keywords}\\b)(\\s*:)?(\\s+)?(\\S+)?/i`: the margin allows at
  most one space between `#` and the keyword, matching the regex's `# ?`,
  but the exact backtracking RuboCop's regex engine performs when several
  keywords overlap as substrings of each other is only approximated by
  trying keywords longest-first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let annotation_keywords = match options.peer("Style/CommentAnnotation", "Keywords") {
            Some(value) => value.to_string_list(),
            None => DEFAULT_ANNOTATION_KEYWORDS.iter().map(|s| (*s).to_string()).collect(),
        };
        Ok(Self {
            allowed_constants: options.str_list("AllowedConstants"),
            annotation_keywords,
            ancestors: Vec::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.ancestors.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::ClassNode => {
                let class = node.as_class_node().expect("kind matched");
                let constant_path = class.constant_path();
                let body = class.body();
                let own_line = ctx.line_col(class.location().span().start).line;
                let qualified = qualify(ctx, &constant_path);
                // RuboCop's `on_class`: `return unless node.body`.
                if let Some(body) = &body {
                    self.check(ctx, node, "class", &constant_path, Some(body), own_line);
                }
                self.ancestors.push(qualified);
            }
            NodeKind::ModuleNode => {
                let module = node.as_module_node().expect("kind matched");
                let constant_path = module.constant_path();
                let body = module.body();
                let own_line = ctx.line_col(module.location().span().start).line;
                let qualified = qualify(ctx, &constant_path);
                self.check(ctx, node, "module", &constant_path, body.as_ref(), own_line);
                self.ancestors.push(qualified);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if matches!(node.kind(), NodeKind::ClassNode | NodeKind::ModuleNode) {
            self.ancestors.pop();
        }
    }
}

/// RuboCop's `qualify_const`: `node.source` for `cbase`/`self`/`call`/local
/// or global/instance/class-variable nodes, else the recursively qualified
/// namespace joined with the constant's own name.
fn qualify(ctx: &Context<'_>, node: &Node<'_>) -> String {
    if let Some(path) = node.as_constant_path_node() {
        let name = String::from_utf8_lossy(ctx.text(path.name_loc().span())).into_owned();
        match path.parent() {
            Some(parent) => format!("{}::{name}", qualify(ctx, &parent)),
            // RuboCop's `cbase` case: a top-level `::Name` path has no parent.
            None => format!("::{name}"),
        }
    } else {
        String::from_utf8_lossy(ctx.text(node.span())).into_owned()
    }
}

/// RuboCop's `node.identifier.short_name`: the last segment of a constant
/// path, or the whole name for a bare constant.
fn short_name(ctx: &Context<'_>, node: &Node<'_>) -> String {
    if let Some(path) = node.as_constant_path_node() {
        String::from_utf8_lossy(ctx.text(path.name_loc().span())).into_owned()
    } else {
        String::from_utf8_lossy(ctx.text(node.span())).into_owned()
    }
}

/// Replaces the first occurrence of `from` with `to`, matching Ruby's
/// `String#sub`. RuboCop uses this to collapse `"Outer" + "::" + "::Name"`
/// (an ancestor joined with a `cbase` name) into a single `::`.
fn replace_first(s: &str, from: &str, to: &str) -> String {
    match s.find(from) {
        Some(idx) => {
            let mut out = String::with_capacity(s.len());
            out.push_str(&s[..idx]);
            out.push_str(to);
            out.push_str(&s[idx + from.len()..]);
            out
        }
        None => s.to_string(),
    }
}

/// RuboCop's `namespace?`: an empty/absent body is not a namespace; a body
/// of two or more statements is a namespace when every statement is a
/// constant definition or visibility declaration; a single-statement body
/// (RuboCop's whitequark AST never wraps a lone statement in a `begin`,
/// unlike Prism's `StatementsNode`, which always wraps) is a namespace only
/// when that one statement is itself a constant definition.
fn is_namespace(body: Option<&Node<'_>>) -> bool {
    let Some(body) = body else { return false };
    let Some(statements) = body.as_statements_node() else { return false };
    let items: Vec<Node<'_>> = statements.body().iter().collect();
    match items.len() {
        0 => false,
        1 => is_constant_definition(&items[0]),
        _ => items.iter().all(is_constant_declaration),
    }
}

/// RuboCop's `constant_declaration?`.
fn is_constant_declaration(node: &Node<'_>) -> bool {
    is_constant_definition(node) || is_constant_visibility_declaration(node)
}

/// RuboCop's `constant_definition?`: matches whitequark's `{class module
/// casgn}` node types. `casgn` covers only plain constant assignment
/// (`A = value`, `A::B = value`), not compound assignment forms.
fn is_constant_definition(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::ClassNode
            | NodeKind::ModuleNode
            | NodeKind::ConstantWriteNode
            | NodeKind::ConstantPathWriteNode
    )
}

/// RuboCop's `constant_visibility_declaration?`:
/// `(send nil? {:public_constant :private_constant} ({sym str} _))`.
fn is_constant_visibility_declaration(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.receiver().is_some() {
        return false;
    }
    let Some(message) = call.message_loc() else { return false };
    let name = message.as_slice();
    if name != b"public_constant" && name != b"private_constant" {
        return false;
    }
    let Some(arguments) = call.arguments() else { return false };
    let args = arguments.arguments();
    if args.len() != 1 {
        return false;
    }
    matches!(
        args.first().expect("length checked").kind(),
        NodeKind::SymbolNode | NodeKind::StringNode
    )
}

/// RuboCop's `include_statement?`:
/// `(send nil? {:include :extend :prepend} const)`.
fn is_include_statement(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.receiver().is_some() {
        return false;
    }
    let Some(message) = call.message_loc() else { return false };
    let name = message.as_slice();
    if name != b"include" && name != b"extend" && name != b"prepend" {
        return false;
    }
    let Some(arguments) = call.arguments() else { return false };
    let args = arguments.arguments();
    if args.len() != 1 {
        return false;
    }
    matches!(
        args.first().expect("length checked").kind(),
        NodeKind::ConstantReadNode | NodeKind::ConstantPathNode
    )
}

/// RuboCop's `include_statement_only?`, ported for Prism's `StatementsNode`
/// wrapping (see [`is_namespace`]): a node passes when it is itself an
/// include/extend/prepend call, or when it is a `StatementsNode` all of
/// whose statements recursively pass.
fn is_include_statement_only(node: &Node<'_>) -> bool {
    if is_include_statement(node) {
        return true;
    }
    match node.as_statements_node() {
        Some(statements) => statements.body().iter().all(|s| is_include_statement_only(&s)),
        None => false,
    }
}

/// Entry point for [`is_include_statement_only`] over a `class`/`module`
/// body, which is `None` for an empty body.
fn include_statement_only_body(body: Option<&Node<'_>>) -> bool {
    match body {
        Some(body) => is_include_statement_only(body),
        None => false,
    }
}

/// A comment that is the only non-whitespace content on its line (RuboCop's
/// leading, as opposed to trailing, comment association).
fn full_line_comment(ctx: &Context<'_>, line: u32) -> Option<linter::CommentInfo> {
    let comment = *ctx.comments().iter().find(|c| c.line == line)?;
    let line_span = ctx.line_span(line);
    let prefix = ctx.text(Span::new(line_span.start, comment.span.start));
    if prefix.iter().all(|&b| b == b' ' || b == b'\t') {
        Some(comment)
    } else {
        None
    }
}

/// RuboCop's `interpreter_directive_comment?`:
/// `/^#\s*(frozen_string_literal|encoding):/`.
fn is_interpreter_directive(text: &[u8]) -> bool {
    let Some(rest) = text.strip_prefix(b"#") else { return false };
    let rest = trim_leading_ws(rest);
    rest.starts_with(b"frozen_string_literal:") || rest.starts_with(b"encoding:")
}

/// RuboCop's `rubocop_directive_comment?`: true when a `# rubocop:disable
/// /enable/todo` directive was parsed from the comment on `line`.
fn is_rubocop_directive(ctx: &Context<'_>, line: u32) -> bool {
    ctx.directives().directives().iter().any(|d| d.line == line)
}

/// True at a byte that can continue a `\w` identifier (ASCII, matching
/// Ruby's default `\b`/`\w` for the ASCII cop-annotation keywords).
fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Ruby's default (non-Unicode) `\s`: space, tab, newline, CR, FF, VT.
fn is_regex_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0B | 0x0C)
}

fn trim_leading_ws(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|&b| !is_regex_ws(b)).unwrap_or(bytes.len());
    &bytes[start..]
}

/// Ruby's `String#capitalize`: first character upcased, the rest
/// downcased.
fn ruby_capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
    }
}

/// RuboCop's `AnnotationComment#annotation?`, ported from
/// `/^(# ?)(\b#{keywords}\b)(\s*:)?(\s+)?(\S+)?/i`: `keyword_appearance?`
/// (a keyword followed by a colon or whitespace) and not
/// `just_keyword_of_sentence?` (a capitalized keyword mid-sentence, with no
/// colon, that reads as plain English rather than a tag).
fn is_annotation(text: &[u8], keywords: &[String]) -> bool {
    let Some(rest) = text.strip_prefix(b"#") else { return false };
    let rest = rest.strip_prefix(b" ").unwrap_or(rest);
    let Ok(rest_str) = std::str::from_utf8(rest) else { return false };

    let mut candidates: Vec<&String> = keywords.iter().filter(|k| !k.is_empty()).collect();
    candidates.sort_by_key(|k| std::cmp::Reverse(k.len()));
    let Some(keyword) = candidates.into_iter().find(|kw| {
        rest_str.len() >= kw.len()
            && rest_str.as_bytes()[..kw.len()].eq_ignore_ascii_case(kw.as_bytes())
            && rest_str.as_bytes().get(kw.len()).is_none_or(|&b| !is_word_byte(b))
    }) else {
        return false;
    };

    let keyword_text = &rest_str[..keyword.len()];
    let after = &rest_str.as_bytes()[keyword.len()..];

    // `(\s*:)?`: the colon group only consumes anything when a `:` is
    // actually found after the whitespace run it allows; when it isn't
    // found, the group matches empty (regex backtracking) rather than
    // eating the whitespace, which stays available for the `(\s+)?` group
    // below.
    let mut ws_run = 0;
    while ws_run < after.len() && is_regex_ws(after[ws_run]) {
        ws_run += 1;
    }
    let colon_present = ws_run < after.len() && after[ws_run] == b':';
    let mut idx = if colon_present { ws_run + 1 } else { 0 };

    let space_start = idx;
    while idx < after.len() && is_regex_ws(after[idx]) {
        idx += 1;
    }
    let space_present = idx > space_start;

    let note_present = idx < after.len() && !is_regex_ws(after[idx]);

    if !(colon_present || space_present) {
        return false;
    }

    let just_keyword_of_sentence = ruby_capitalize(keyword_text) == keyword_text
        && !colon_present
        && space_present
        && note_present;
    !just_keyword_of_sentence
}

/// RuboCop's `nodoc?`: `/^#\s*:nodoc:#{"\s+all\s*$" if require_all}/`.
fn is_nodoc(text: &[u8], require_all: bool) -> bool {
    let Some(rest) = text.strip_prefix(b"#") else { return false };
    let rest = trim_leading_ws(rest);
    let Some(rest) = rest.strip_prefix(b":nodoc:") else { return false };
    if !require_all {
        return true;
    }
    let mut idx = 0;
    while idx < rest.len() && is_regex_ws(rest[idx]) {
        idx += 1;
    }
    if idx == 0 {
        return false;
    }
    let Some(after_all) = rest[idx..].strip_prefix(b"all") else { return false };
    after_all.iter().all(|&b| is_regex_ws(b))
}

/// True when the comment on `line`, if any, is a `:nodoc:` comment
/// (RuboCop's `same_line?(nodoc, node) && nodoc?(nodoc, require_all:)`).
fn nodoc_matches_line(ctx: &Context<'_>, line: u32, require_all: bool) -> bool {
    match ctx.comments().iter().find(|c| c.line == line) {
        Some(comment) => is_nodoc(ctx.text(comment.span), require_all),
        None => false,
    }
}

/// RuboCop's `nodoc_self_or_outer_module?`, minus the `compact_namespace?`
/// branch (see `META.blind_spots`): true when the node's own declaration
/// line carries a bare `#:nodoc:` (or `#:nodoc: all`), or any actual
/// `class`/`module` ancestor's declaration line carries `#:nodoc: all`.
/// Ancestor declaration lines are read from `ctx.ancestors()` (filtered to
/// `ClassNode`/`ModuleNode`) rather than tracked separately by the rule.
fn nodoc_applies(ctx: &Context<'_>, own_line: u32) -> bool {
    if nodoc_matches_line(ctx, own_line, false) {
        return true;
    }
    ctx.ancestors()
        .iter()
        .filter(|a| matches!(a.kind, NodeKind::ClassNode | NodeKind::ModuleNode))
        .any(|a| nodoc_matches_line(ctx, ctx.line_col(a.span.start).line, true))
}
