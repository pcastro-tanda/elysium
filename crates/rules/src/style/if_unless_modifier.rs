//! `Style/IfUnlessModifier`, ported from RuboCop's
//! `lib/rubocop/cop/style/if_unless_modifier.rb` (plus the `StatementModifier`,
//! `LineLengthHelp` and `AllowedPattern` mixins it includes).

use std::collections::{HashMap, HashSet};

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::{CallNode, Location, StatementsNode};
use ruby_ast::{each_descendant, for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{char_len, Span};

/// RuboCop's `MSG_USE_MODIFIER`.
const MSG_USE_MODIFIER: &str = "Favor modifier `{keyword}` usage when having a single-line body. \
Another good alternative is the usage of control flow `&&`/`||`.";

/// RuboCop's `MSG_USE_MODIFIER_PARENS`.
const MSG_USE_MODIFIER_PARENS: &str = "Favor modifier `{keyword}` usage when having a \
single-line body. Wrap the expression in parentheses to keep the current behavior, as it is \
part of a larger expression.";

/// RuboCop's `MSG_USE_NORMAL`.
const MSG_USE_NORMAL: &str = "Modifier form of `{keyword}` makes the line too long.";

/// Checks for `if`/`unless` statements that would fit on one line as a
/// modifier, and for modifier `if`/`unless` lines that are too long.
#[derive(Debug, Clone)]
pub struct IfUnlessModifier {
    /// `Layout/LineLength`'s `Max`, or `None` when that cop is disabled.
    max_line_length: Option<i64>,
    /// `Layout/LineLength`'s `AllowURI`.
    allow_uri: bool,
    /// `Layout/LineLength`'s `AllowCopDirectives`.
    allow_cop_directives: bool,
    /// Regex matching a URI at the point `AllowURI` starts caring, built
    /// from `Layout/LineLength`'s `URISchemes`.
    uri_regex: Option<Regex>,
    /// `Layout/LineLength`'s `AllowedPatterns`/`IgnoredPatterns`, compiled.
    allowed_patterns: Vec<Regex>,
    /// `Layout/IndentationStyle`'s `IndentationWidth`, else
    /// `Layout/IndentationWidth`'s `Width`, else `2` (RuboCop's
    /// `tab_indentation_width`); used to weigh a line's leading tabs as
    /// this many columns each when checking line length.
    tab_indentation_width: i64,

    /// For each direct item of a `StatementsNode` body, the names assigned
    /// (via a bare `x = value` statement) by every earlier item in the same
    /// list. Mirrors RuboCop's `Node#left_siblings`.
    left_assigned_names: HashMap<Span, Vec<Box<str>>>,
    /// For each direct item of a `StatementsNode` body, the first line of
    /// the following item, if any. Mirrors `another_statement_on_same_line?`.
    next_sibling_line: HashMap<Span, u32>,
    /// Spans that are exactly the receiver of some `CallNode` (RuboCop's
    /// `Node#chained?`).
    chained_receivers: HashSet<Span>,
    /// Spans whose parent shape means an `if`/`unless` there needs
    /// parentheses when written in modifier form (RuboCop's `parenthesize?`).
    paren_targets: HashSet<Span>,
    /// Spans of every whitequark-`lvasgn`-equivalent node seen so far this
    /// file (`LocalVariableWriteNode`, the `+=`/`&&=`/`||=` operator-write
    /// variants, and `LocalVariableTargetNode` for multiple-assignment
    /// targets), in source order (RuboCop's `non_eligible_condition?`).
    lvasgn_spans: Vec<Span>,
    /// Spans of every `MatchPredicateNode`/`MatchRequiredNode` seen so far
    /// this file, in source order (RuboCop's `pattern_matching_nodes`).
    match_pattern_spans: Vec<Span>,
    /// Spans of every `IfNode`/`UnlessNode` seen so far this file, in
    /// source order (RuboCop's `nested_conditional?`).
    conditional_spans: Vec<Span>,
    /// `(span, name)` for every `defined?(lvar)`/`defined?(call)` seen so
    /// far this file, in source order (RuboCop's `defined_nodes`).
    defined_calls: Vec<(Span, Box<[u8]>)>,
}

impl IfUnlessModifier {
    /// Reads another cop's boolean option, falling back to `default` when
    /// unconfigured (peer reads have no schema-driven default).
    fn peer_bool(options: &RuleOptions, cop: &str, key: &str, default: bool) -> bool {
        options.peer(cop, key).and_then(OptionValue::as_bool).unwrap_or(default)
    }

    /// Reads another cop's integer option, falling back to `default`.
    fn peer_int(options: &RuleOptions, cop: &str, key: &str, default: i64) -> i64 {
        options.peer(cop, key).and_then(OptionValue::as_int).unwrap_or(default)
    }

    /// Reads another cop's string-list option, falling back to `default`.
    fn peer_str_list(options: &RuleOptions, cop: &str, key: &str, default: &[&str]) -> Vec<String> {
        match options.peer(cop, key) {
            Some(value) => value.to_string_list(),
            None => default.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// RuboCop's `line_length`: character count plus the extra columns
    /// contributed by leading tabs.
    fn line_length(&self, line: &[u8]) -> i64 {
        i64::from(char_len(line)) + self.indentation_difference(line)
    }

    /// RuboCop's `indentation_difference`: leading-tab count times
    /// `tab_indentation_width - 1` (a tab counts as one byte/char but
    /// `tab_indentation_width` display columns).
    fn indentation_difference(&self, line: &[u8]) -> i64 {
        let leading_tabs = line.iter().take_while(|&&b| b == b'\t').count();
        i64::try_from(leading_tabs).unwrap_or(i64::MAX) * (self.tab_indentation_width - 1)
    }
}

impl Rule for IfUnlessModifier {
    const META: RuleMeta = RuleMeta {
        name: "Style/IfUnlessModifier",
        department: Department::Style,
        summary: "Favor modifier if/unless usage when you have a single-line body.",
        explanation: "\
Checks for `if` and `unless` statements that would fit on one line if
written as modifier `if`/`unless`. The cop also checks for modifier
`if`/`unless` lines that exceed the maximum line length.

The maximum line length is configured in the `Layout/LineLength` cop.

One-line pattern matching is always allowed, since the match variable would
become undefined if the code were changed to the modifier form:

```ruby
if [42] in [x]
  x # `x` is undefined when using modifier form.
end
```

To respect the user's intention to use an endless method definition in the
`if` body, the following code is allowed:

```ruby
if condition
  def method_name = body
end
```

It is also allowed when a `defined?` argument has an undefined value, because
using the modifier form would change its result:

```ruby
unless defined?(undefined_foo)
  undefined_foo = 'default_value'
end
undefined_foo # => 'default_value'

undefined_bar = 'default_value' unless defined?(undefined_bar)
undefined_bar # => nil
```

```ruby
# bad
if condition
  do_stuff(bar)
end

unless qux.empty?
  Foo.do_something
end

do_something_with_a_long_name(arg) if long_condition_that_prevents_code_fit_on_single_line

# good
do_stuff(bar) if condition
Foo.do_something unless qux.empty?

if long_condition_that_prevents_code_fit_on_single_line
  do_something_with_a_long_name(arg)
end

if short_condition # a long comment that makes it too long if it were just a single line
  do_something
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::IfNode,
            NodeKind::UnlessNode,
            NodeKind::StatementsNode,
            NodeKind::CallNode,
            NodeKind::AndNode,
            NodeKind::OrNode,
            NodeKind::ArrayNode,
            NodeKind::AssocNode,
            NodeKind::LocalVariableWriteNode,
            NodeKind::LocalVariableOperatorWriteNode,
            NodeKind::LocalVariableAndWriteNode,
            NodeKind::LocalVariableOrWriteNode,
            NodeKind::LocalVariableTargetNode,
            NodeKind::InstanceVariableWriteNode,
            NodeKind::InstanceVariableOperatorWriteNode,
            NodeKind::InstanceVariableAndWriteNode,
            NodeKind::InstanceVariableOrWriteNode,
            NodeKind::ClassVariableWriteNode,
            NodeKind::ClassVariableOperatorWriteNode,
            NodeKind::ClassVariableAndWriteNode,
            NodeKind::ClassVariableOrWriteNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::GlobalVariableOperatorWriteNode,
            NodeKind::GlobalVariableAndWriteNode,
            NodeKind::GlobalVariableOrWriteNode,
            NodeKind::ConstantWriteNode,
            NodeKind::ConstantOperatorWriteNode,
            NodeKind::ConstantAndWriteNode,
            NodeKind::ConstantOrWriteNode,
            NodeKind::ConstantPathWriteNode,
            NodeKind::ConstantPathOperatorWriteNode,
            NodeKind::ConstantPathAndWriteNode,
            NodeKind::ConstantPathOrWriteNode,
            NodeKind::IndexOperatorWriteNode,
            NodeKind::IndexAndWriteNode,
            NodeKind::IndexOrWriteNode,
            NodeKind::CallOperatorWriteNode,
            NodeKind::CallAndWriteNode,
            NodeKind::CallOrWriteNode,
            NodeKind::MultiWriteNode,
            NodeKind::MatchPredicateNode,
            NodeKind::MatchRequiredNode,
            NodeKind::DefinedNode,
        ],
        config: &[],
        blind_spots: "\
Reads `Layout/LineLength`'s `Max`/`Enabled`/`AllowURI`/`AllowCopDirectives`/
`URISchemes`/`AllowedPatterns`/`IgnoredPatterns`, and
`Layout/IndentationStyle`'s `IndentationWidth`/`Layout/IndentationWidth`'s
`Width` (for weighing a line's leading tabs), all as peer options.

`Node#left_siblings` (used for the `defined?` guard and
`another_statement_on_same_line?`) and `Node#chained?`/`parenthesize?` (used
for the `chained?` guard, the parenthesizing message, and fix
parenthesization) are reconstructed from a `StatementsNode`'s direct body
list and the known wrapping constructs (every assignment kind, including
multiple, index, attribute and `+=`/`||=`-style operator assignments,
`&&`/`||`, array elements, hash values, call receiver/arguments) rather than
true parent pointers; an `if`/`unless` outside a `StatementsNode` body is
treated as having no left siblings, which only risks false negatives.

`if_body_source`'s omitted-hash-value reconstruction only special-cases a
call whose last argument is a hash/keyword-hash with a value-omitted last
pair (`obj.foo bar:`); other RuboCop-recognized shapes for that rewrite fall
back to the body's raw source, which is usually byte-identical anyway.

The heredoc-aware block-form correction (and `XStringNode`/
`InterpolatedXStringNode` heredocs) assumes a single-line heredoc body,
matching RuboCop's own `to_normal_form_with_heredoc`, which does not
re-indent a multi-line heredoc body per line either; only plain string and
interpolated-string heredocs are recognized as a call's last argument.

`AllowURI`'s URI matching is a simplified `scheme://\\S+` regex rather than
`URI::DEFAULT_PARSER.make_regexp` plus RuboCop's YARD-link end-position
extension; it accepts the same URIs in the common case (a bare URI running
to the end of the line) but does not replicate the `{<uri> <title>}` or
trailing-word extensions.

`self.autocorrect_incompatible_with` (`Style::Next`/`Style::SoleNestedConditional`)
is not ported: this port has no cross-rule autocorrect-conflict mechanism.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let line_length_enabled = Self::peer_bool(options, "Layout/LineLength", "Enabled", true);
        let max_line_length = if line_length_enabled {
            Some(Self::peer_int(options, "Layout/LineLength", "Max", 120))
        } else {
            None
        };
        let allow_uri = Self::peer_bool(options, "Layout/LineLength", "AllowURI", true);
        let allow_cop_directives =
            Self::peer_bool(options, "Layout/LineLength", "AllowCopDirectives", true);
        let schemes =
            Self::peer_str_list(options, "Layout/LineLength", "URISchemes", &["http", "https"]);
        let uri_regex = if schemes.is_empty() {
            None
        } else {
            let alternation =
                schemes.iter().map(|s| regex::escape(s)).collect::<Vec<_>>().join("|");
            Regex::new(&format!("(?:{alternation})://\\S+")).ok()
        };
        let mut allowed_patterns = Vec::new();
        for pattern in Self::peer_str_list(options, "Layout/LineLength", "AllowedPatterns", &[])
            .into_iter()
            .chain(Self::peer_str_list(options, "Layout/LineLength", "IgnoredPatterns", &[]))
        {
            if let Ok(re) = Regex::new(&pattern) {
                allowed_patterns.push(re);
            }
        }
        let tab_indentation_width = options
            .peer("Layout/IndentationStyle", "IndentationWidth")
            .and_then(OptionValue::as_int)
            .unwrap_or_else(|| Self::peer_int(options, "Layout/IndentationWidth", "Width", 2));
        Ok(Self {
            max_line_length,
            allow_uri,
            allow_cop_directives,
            uri_regex,
            allowed_patterns,
            tab_indentation_width,
            left_assigned_names: HashMap::new(),
            next_sibling_line: HashMap::new(),
            chained_receivers: HashSet::new(),
            paren_targets: HashSet::new(),
            lvasgn_spans: Vec::new(),
            match_pattern_spans: Vec::new(),
            conditional_spans: Vec::new(),
            defined_calls: Vec::new(),
        })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.left_assigned_names.clear();
        self.next_sibling_line.clear();
        self.chained_receivers.clear();
        self.paren_targets.clear();
        self.lvasgn_spans.clear();
        self.match_pattern_spans.clear();
        self.conditional_spans.clear();
        self.defined_calls.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::StatementsNode { .. } => self.record_statements(node, ctx),
            Node::CallNode { .. } => self.record_call(node),
            Node::AndNode { .. } => {
                let n = node.as_and_node().expect("kind matched");
                self.paren_targets.insert(n.left().span());
                self.paren_targets.insert(n.right().span());
            }
            Node::OrNode { .. } => {
                let n = node.as_or_node().expect("kind matched");
                self.paren_targets.insert(n.left().span());
                self.paren_targets.insert(n.right().span());
            }
            Node::ArrayNode { .. } => {
                let n = node.as_array_node().expect("kind matched");
                for element in &n.elements() {
                    self.paren_targets.insert(element.span());
                }
            }
            Node::AssocNode { .. } => {
                let n = node.as_assoc_node().expect("kind matched");
                self.paren_targets.insert(n.value().span());
            }
            Node::LocalVariableWriteNode { .. } => {
                let n = node.as_local_variable_write_node().expect("kind matched");
                self.paren_targets.insert(n.value().span());
                self.lvasgn_spans.push(node.span());
            }
            Node::LocalVariableOperatorWriteNode { .. }
            | Node::LocalVariableAndWriteNode { .. }
            | Node::LocalVariableOrWriteNode { .. } => {
                // RuboCop's `non_eligible_condition?` (`lvasgn_type?`) also
                // matches these: whitequark desugars every compound local
                // assignment (`+=`/`&&=`/`||=`) into the same bare `lvasgn`
                // node as plain `=`.
                self.lvasgn_spans.push(node.span());
                if let Some(value) = assignment_value(node) {
                    self.paren_targets.insert(value.span());
                }
            }
            Node::LocalVariableTargetNode { .. } => {
                // whitequark represents each target of a multiple
                // assignment (`w, h = ...`) as its own bare `lvasgn` node
                // (no value child), so `non_eligible_condition?` sees one
                // per target; Prism groups them under `MultiWriteNode`, so
                // this is the equivalent per-target node to record.
                self.lvasgn_spans.push(node.span());
            }
            Node::MatchPredicateNode { .. } | Node::MatchRequiredNode { .. } => {
                self.match_pattern_spans.push(node.span());
            }
            Node::DefinedNode { .. } => {
                let defined = node.as_defined_node().expect("kind matched");
                let argument = defined.value();
                let name: Option<&[u8]> = match &argument {
                    Node::LocalVariableReadNode { .. } => Some(
                        argument
                            .as_local_variable_read_node()
                            .expect("kind matched")
                            .name()
                            .as_slice(),
                    ),
                    Node::CallNode { .. } => {
                        Some(argument.as_call_node().expect("kind matched").name().as_slice())
                    }
                    _ => None,
                };
                if let Some(name) = name {
                    self.defined_calls.push((node.span(), name.to_vec().into_boxed_slice()));
                }
            }
            Node::IfNode { .. } | Node::UnlessNode { .. } => {
                self.conditional_spans.push(node.span());
            }
            _ => {
                // Every other assignment kind ([`assignment_value`]): RuboCop's
                // `parent.assignment?` covers `ivasgn`/`cvasgn`/`gvasgn`/
                // `casgn`/`masgn` and every `op_asgn`/`or_asgn`/`and_asgn`,
                // whatever their target (`@x ||= if ...`, `h[k] ||= if ...`).
                if let Some(value) = assignment_value(node) {
                    self.paren_targets.insert(value.span());
                }
            }
        }
    }
    /// Runs the actual `if`/`unless` checks after the node's own condition
    /// and body -- including any nested conditionals, pattern matches,
    /// local-variable writes, and `defined?` calls they contain, recorded
    /// by `enter` above -- have been fully visited, so the various
    /// `contains X anywhere in this subtree` checks can query the recorded
    /// spans instead of walking the subtree here.
    fn leave(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node {
            Node::IfNode { .. } => self.check_if(node, ctx),
            Node::UnlessNode { .. } => self.check_unless(node, ctx),
            _ => {}
        }
    }
}

impl IfUnlessModifier {
    /// Records, for every direct item of this `StatementsNode`'s body, the
    /// names assigned by earlier items (RuboCop's `left_siblings` filtered
    /// to bare `lvasgn` statements) and the next item's first line
    /// (RuboCop's `another_statement_on_same_line?`).
    fn record_statements(&mut self, node: &Node<'_>, ctx: &Context<'_>) {
        let stmts = node.as_statements_node().expect("kind matched");
        let items: Vec<Node<'_>> = stmts.body().iter().collect();
        let mut names_so_far: Vec<Box<str>> = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let span = item.span();
            self.left_assigned_names.insert(span, names_so_far.clone());
            if let Some(next) = items.get(index + 1) {
                self.next_sibling_line.insert(span, ctx.line_col(next.span().start).line);
            }
            if let Node::LocalVariableWriteNode { .. } = item {
                let write = item.as_local_variable_write_node().expect("kind matched");
                let name = String::from_utf8_lossy(write.name().as_slice()).into_owned();
                names_so_far.push(name.into_boxed_str());
            }
        }
    }

    /// Records a `CallNode`'s receiver (for `chained?`) and every operand
    /// (receiver and arguments, for `parenthesize?`'s `parent.send_type?`).
    fn record_call(&mut self, node: &Node<'_>) {
        let call = node.as_call_node().expect("kind matched");
        if let Some(receiver) = call.receiver() {
            self.chained_receivers.insert(receiver.span());
            self.paren_targets.insert(receiver.span());
        }
        if let Some(arguments) = call.arguments() {
            for argument in &arguments.arguments() {
                self.paren_targets.insert(argument.span());
            }
        }
    }

    fn check_if(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let if_node = node.as_if_node().expect("kind matched");
        let Some(keyword_loc) = if_node.if_keyword_loc() else {
            // Ternary (`a ? b : c`): its `modifier_form?` is technically
            // `true` (no `end` keyword) but RuboCop's `ternary?` always
            // makes it ineligible for the "convert to modifier" message,
            // and reporting "too long" with an empty keyword is an
            // upstream oddity no fixture exercises. Skip (false negative
            // only).
            return;
        };
        if ctx.text(keyword_loc.span()) == b"elsif" {
            // `elsif` shares its outer chain's `end`, so `modifier_form?`
            // is false, and `non_simple_if_unless?` (`elsif?`) is always
            // true: this node can never produce a message.
            return;
        }
        let condition = if_node.predicate();
        if self.top_level_guard(ctx, &condition, if_node.statements().as_ref(), node.span()) {
            return;
        }
        let is_modifier_form = if_node.end_keyword_loc().is_none();
        if is_modifier_form {
            let Some(body_stmt) = if_node.statements().and_then(|s| s.body().first()) else {
                return;
            };
            self.check_too_long_modifier(
                ctx,
                node.span(),
                keyword_loc.span(),
                "if",
                &condition,
                &body_stmt,
            );
        } else {
            if if_node.subsequent().is_some() || condition_has_match_write(&condition) {
                return;
            }
            self.check_convertible(
                ctx,
                node.span(),
                keyword_loc.span(),
                "if",
                &condition,
                if_node.statements(),
                &if_node.end_keyword_loc().expect("block form has an end"),
            );
        }
    }

    fn check_unless(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let unless_node = node.as_unless_node().expect("kind matched");
        let keyword_span = unless_node.keyword_loc().span();
        let condition = unless_node.predicate();
        if self.top_level_guard(ctx, &condition, unless_node.statements().as_ref(), node.span()) {
            return;
        }
        let is_modifier_form = unless_node.end_keyword_loc().is_none();
        if is_modifier_form {
            let Some(body_stmt) = unless_node.statements().and_then(|s| s.body().first()) else {
                return;
            };
            self.check_too_long_modifier(
                ctx,
                node.span(),
                keyword_span,
                "unless",
                &condition,
                &body_stmt,
            );
        } else {
            if unless_node.else_clause().is_some() {
                return;
            }
            self.check_convertible(
                ctx,
                node.span(),
                keyword_span,
                "unless",
                &condition,
                unless_node.statements(),
                &unless_node.end_keyword_loc().expect("block form has an end"),
            );
        }
    }

    /// RuboCop's top-of-`on_if` guards, common to both branches: the node
    /// (or an ancestor) is inside a string interpolation, an endless-method
    /// body, or a `defined?`/pattern-matching condition that would change
    /// meaning in modifier form.
    fn top_level_guard(
        &self,
        ctx: &Context<'_>,
        condition: &Node<'_>,
        body: Option<&StatementsNode<'_>>,
        own_span: Span,
    ) -> bool {
        if ctx.ancestors().iter().any(|a| a.kind == NodeKind::InterpolatedStringNode) {
            return true;
        }
        if is_endless_def_body(body) {
            return true;
        }
        if self.has_match_pattern(condition.span()) {
            return true;
        }
        let empty = Vec::new();
        let left_names = self.left_assigned_names.get(&own_span).unwrap_or(&empty);
        self.any_defined_is_undefined(condition.span(), left_names)
    }

    /// RuboCop's `pattern_matching_nodes(condition).any?`: `condition` or
    /// any descendant is `foo in pattern` / `foo => pattern`, found via the
    /// `match_pattern_spans` recorded by `enter` instead of a subtree walk.
    fn has_match_pattern(&self, condition_span: Span) -> bool {
        any_span_within(&self.match_pattern_spans, condition_span)
    }

    /// RuboCop's `non_eligible_condition?`: any descendant (including the
    /// condition itself) is a bare local variable assignment.
    fn has_lvasgn(&self, condition_span: Span) -> bool {
        any_span_within(&self.lvasgn_spans, condition_span)
    }

    /// RuboCop's `node.nested_conditional?`: the body contains (anywhere)
    /// another `if`/`unless`.
    fn has_nested_conditional(&self, body: Option<&StatementsNode<'_>>) -> bool {
        let Some(body) = body else { return false };
        any_span_within(&self.conditional_spans, body.location().span())
    }

    /// RuboCop's `defined_nodes(condition).any? { defined_argument_is_undefined? }`.
    fn any_defined_is_undefined(&self, condition_span: Span, left_names: &[Box<str>]) -> bool {
        names_within(&self.defined_calls, condition_span)
            .any(|name| !left_names.iter().any(|assigned| assigned.as_bytes() == name))
    }

    /// RuboCop's `too_long_due_to_modifier?` plus the offense/fix for the
    /// already-modifier-form node.
    fn check_too_long_modifier(
        &self,
        ctx: &mut Context<'_>,
        node_span: Span,
        keyword_span: Span,
        keyword: &'static str,
        condition: &Node<'_>,
        body_stmt: &Node<'_>,
    ) {
        let first_line = ctx.line_col(node_span.start).line;
        if let Some(&next_line) = self.next_sibling_line.get(&node_span) {
            if next_line == first_line {
                return;
            }
        }
        if !self.too_long_single_line(ctx, node_span, first_line) {
            return;
        }
        let message = MSG_USE_NORMAL.replace("{keyword}", keyword);
        if another_modifier_if_on_same_line(ctx, node_span, first_line) {
            // RuboCop's `next if another_modifier_if_on_same_line?(node)`:
            // report but skip the fix, since converting only this one to
            // block form would strand its sibling modifier-form `if` mid
            // expression.
            ctx.report(&Self::META, keyword_span, message);
            return;
        }
        let indent = " ".repeat(ctx.line_col(node_span.start).column as usize);
        let fix = self.modifier_too_long_fix(
            ctx, node_span, keyword, condition, body_stmt, first_line, &indent,
        );
        ctx.report_with_fix(&Self::META, keyword_span, message, fix);
    }

    /// RuboCop's `too_long_single_line?`.
    fn too_long_single_line(&self, ctx: &Context<'_>, node_span: Span, first_line: u32) -> bool {
        let Some(max) = self.max_line_length else { return false };
        let last_line = ctx.line_col(node_span.end.saturating_sub(1)).line;
        if first_line != last_line {
            return false;
        }
        if ctx.directives().is_disabled("Layout/LineLength", first_line) {
            return false;
        }
        let line = ctx.line_text(first_line);
        if self.line_length(line) <= max {
            return false;
        }
        self.too_long_line_based_on_config(ctx, first_line, line, max)
    }

    fn too_long_line_based_on_config(
        &self,
        ctx: &Context<'_>,
        line_no: u32,
        line: &[u8],
        max: i64,
    ) -> bool {
        if matches_allowed_pattern(&self.allowed_patterns, line) {
            return false;
        }
        if self.allow_cop_directives {
            if let Some(directive) =
                ctx.directives().directives().iter().find(|d| d.line == line_no)
            {
                let line_start = ctx.line_span(line_no).start;
                let rel = usize::try_from(directive.span.start.saturating_sub(line_start))
                    .unwrap_or(0)
                    .min(line.len());
                let truncated = trim_end_ascii_space(&line[..rel]);
                return self.line_length(truncated) > max;
            }
        }
        self.too_long_based_on_uri(line, max)
    }

    fn too_long_based_on_uri(&self, line: &[u8], max: i64) -> bool {
        if !self.allow_uri {
            return true;
        }
        let Some(re) = &self.uri_regex else { return true };
        let Ok(text) = std::str::from_utf8(line) else { return true };
        let Some(m) = re.find_iter(text).last() else { return true };
        let diff = self.indentation_difference(line);
        let begin = i64::from(char_len(&line[..m.start()])) + diff;
        let end = i64::from(char_len(&line[..m.end()])) + diff;
        !(begin < max && end == self.line_length(line))
    }

    /// Builds the fix for an already-modifier-form node whose line is too
    /// long: either move a trailing comment to its own line above (when
    /// that alone explains the overflow, RuboCop's
    /// `too_long_due_to_comment_after_modifier?`), or convert to block
    /// form.
    #[allow(clippy::too_many_arguments)]
    fn modifier_too_long_fix(
        &self,
        ctx: &Context<'_>,
        node_span: Span,
        keyword: &'static str,
        condition: &Node<'_>,
        body_stmt: &Node<'_>,
        first_line: u32,
        indent: &str,
    ) -> Fix {
        if let Some(comment) = ctx.comments().iter().find(|c| c.line == first_line).copied() {
            let line = ctx.line_text(first_line);
            let source_length = self.line_length(line);
            let comment_length = i64::from(char_len(ctx.text(comment.span)));
            let max = self.max_line_length.unwrap_or(120);
            if source_length - comment_length <= max && max <= source_length {
                let line_start = ctx.line_span(first_line).start;
                let mut delete_start = comment.span.start;
                while delete_start > line_start
                    && matches!(ctx.text(Span::new(delete_start - 1, delete_start)), b" " | b"\t")
                {
                    delete_start -= 1;
                }
                let mut replacement = ctx.text(comment.span).to_vec();
                replacement.push(b'\n');
                replacement.extend_from_slice(indent.as_bytes());
                replacement.extend_from_slice(ctx.text(node_span));
                return Fix {
                    applicability: Applicability::Safe,
                    edits: vec![
                        Edit::delete(Span::new(delete_start, comment.span.end)),
                        Edit::replace(node_span, replacement),
                    ],
                };
            }
        }
        Self::to_block_form_fix(ctx, node_span, keyword, condition, body_stmt, indent)
    }

    /// Converts a modifier-form if/unless node's span into block form,
    /// handling a heredoc last argument like RuboCop's
    /// `to_normal_form_with_heredoc`.
    fn to_block_form_fix(
        ctx: &Context<'_>,
        node_span: Span,
        keyword: &'static str,
        condition: &Node<'_>,
        body_stmt: &Node<'_>,
        indent: &str,
    ) -> Fix {
        let mut replacement = Vec::new();
        replacement.extend_from_slice(keyword.as_bytes());
        replacement.push(b' ');
        replacement.extend_from_slice(ctx.text(condition.span()));
        replacement.push(b'\n');
        replacement.extend_from_slice(indent.as_bytes());
        replacement.extend_from_slice(b"  ");
        replacement.extend_from_slice(ctx.text(body_stmt.span()));

        let mut edits = Vec::new();
        if let Some(last_arg) = last_argument_if_call(body_stmt) {
            if let Some((_opening, content_span, closing)) = heredoc_regions(&last_arg) {
                if ruby_ast::ext::is_heredoc(&last_arg) {
                    let content_text = trim_end_newline(ctx.text(content_span));
                    let closing_text = trim_end_newline(ctx.text(closing.span()));
                    replacement.push(b'\n');
                    replacement.extend_from_slice(indent.as_bytes());
                    replacement.extend_from_slice(b"  ");
                    replacement.extend_from_slice(content_text);
                    replacement.push(b'\n');
                    replacement.extend_from_slice(indent.as_bytes());
                    replacement.extend_from_slice(b"  ");
                    replacement.extend_from_slice(closing_text);
                    edits.push(Edit::delete(ctx.whole_lines(content_span)));
                    edits.push(Edit::delete(ctx.whole_lines(closing.span())));
                }
            }
        }
        replacement.push(b'\n');
        replacement.extend_from_slice(indent.as_bytes());
        replacement.extend_from_slice(b"end");
        edits.push(Edit::replace(node_span, replacement));
        Fix { applicability: Applicability::Safe, edits }
    }

    /// RuboCop's `single_line_as_modifier?` plus `named_capture_in_condition?`,
    /// and the offense/fix for the block-form node.
    #[allow(clippy::too_many_arguments)]
    fn check_convertible(
        &self,
        ctx: &mut Context<'_>,
        node_span: Span,
        keyword_span: Span,
        keyword: &'static str,
        condition: &Node<'_>,
        body: Option<StatementsNode<'_>>,
        end_loc: &Location<'_>,
    ) {
        if self.chained_receivers.contains(&node_span) {
            return;
        }
        if self.has_nested_conditional(body.as_ref()) {
            return;
        }
        if nonempty_line_count(ctx, node_span) > 3 {
            return;
        }
        let last_line = ctx.line_col(end_loc.span().start).line;
        if multiline_inside_collection(ctx, node_span, last_line) {
            return;
        }
        if ctx.comments().iter().any(|c| c.line == last_line) {
            return;
        }
        let first_line = ctx.line_col(node_span.start).line;
        let has_first_line_comment = first_line_comment(ctx, first_line).is_some();
        let has_code_after = code_after(ctx, end_loc).is_some();
        if has_first_line_comment && has_code_after {
            return;
        }
        let Some(body) = body else { return };
        let items: Vec<Node<'_>> = body.body().iter().collect();
        if items.len() != 1 {
            return;
        }
        let stmt = &items[0];
        let body_span = body.location().span();
        let body_first_line = ctx.line_col(body_span.start).line;
        let body_last_line = ctx.line_col(body_span.end.saturating_sub(1)).line;
        if ctx.comments().iter().any(|c| c.line >= body_first_line && c.line <= body_last_line) {
            return;
        }
        if self.has_lvasgn(condition.span()) {
            return;
        }
        if !self.modifier_fits_on_single_line(
            ctx,
            node_span,
            keyword_span,
            keyword,
            condition,
            stmt,
            end_loc,
        ) {
            return;
        }
        if condition_has_match_write(condition) {
            return;
        }
        let message = if self.paren_targets.contains(&node_span) {
            MSG_USE_MODIFIER_PARENS.replace("{keyword}", keyword)
        } else {
            MSG_USE_MODIFIER.replace("{keyword}", keyword)
        };
        if another_modifier_if_on_same_line(ctx, node_span, first_line) {
            // RuboCop's `next if another_modifier_if_on_same_line?(node)`:
            // report but skip the fix.
            ctx.report(&Self::META, keyword_span, message);
            return;
        }
        let fix = self.to_modifier_form_fix(ctx, node_span, keyword, condition, stmt);
        ctx.report_with_fix(&Self::META, keyword_span, message, fix);
    }

    /// RuboCop's `modifier_fits_on_single_line?` / `length_in_modifier_form`.
    #[allow(clippy::too_many_arguments)]
    fn modifier_fits_on_single_line(
        &self,
        ctx: &Context<'_>,
        node_span: Span,
        keyword_span: Span,
        keyword: &'static str,
        condition: &Node<'_>,
        body_stmt: &Node<'_>,
        end_loc: &Location<'_>,
    ) -> bool {
        let Some(max) = self.max_line_length else { return true };
        let keyword_line = ctx.line_col(keyword_span.start).line;
        let keyword_col = ctx.line_col(keyword_span.start).column as usize;
        let line_text = ctx.line_text(keyword_line);
        let code_before = &line_text[..keyword_col.min(line_text.len())];
        let expression = self.modifier_expression(ctx, node_span, keyword, condition, body_stmt);
        let after = code_after(ctx, end_loc).unwrap_or(&[]);
        let mut full = code_before.to_vec();
        full.extend_from_slice(&expression);
        full.extend_from_slice(after);
        self.line_length(&full) <= max
    }

    /// RuboCop's `to_modifier_form`.
    fn modifier_expression(
        &self,
        ctx: &Context<'_>,
        node_span: Span,
        keyword: &'static str,
        condition: &Node<'_>,
        body_stmt: &Node<'_>,
    ) -> Vec<u8> {
        let body_source = if_body_source(ctx, body_stmt);
        let mut expr = body_source;
        expr.push(b' ');
        expr.extend_from_slice(keyword.as_bytes());
        expr.push(b' ');
        expr.extend_from_slice(ctx.text(condition.span()));
        let mut out = if self.paren_targets.contains(&node_span) {
            let mut p = Vec::with_capacity(expr.len() + 2);
            p.push(b'(');
            p.extend_from_slice(&expr);
            p.push(b')');
            p
        } else {
            expr
        };
        if let Some(comment) = first_line_comment(ctx, ctx.line_col(node_span.start).line) {
            out.push(b' ');
            out.extend_from_slice(comment);
        }
        out
    }

    fn to_modifier_form_fix(
        &self,
        ctx: &Context<'_>,
        node_span: Span,
        keyword: &'static str,
        condition: &Node<'_>,
        body_stmt: &Node<'_>,
    ) -> Fix {
        let replacement = self.modifier_expression(ctx, node_span, keyword, condition, body_stmt);
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node_span, replacement)],
        }
    }
}

/// RuboCop's `endless_method?`: the body is exactly one endless `def`.
fn is_endless_def_body(body: Option<&StatementsNode<'_>>) -> bool {
    let Some(body) = body else { return false };
    let list = body.body();
    if list.len() != 1 {
        return false;
    }
    let Some(item) = list.first() else { return false };
    let Node::DefNode { .. } = &item else { return false };
    item.as_def_node().expect("kind matched").equal_loc().is_some()
}

/// RuboCop's `named_capture_in_condition?`: `condition.match_with_lvasgn_type?`.
fn condition_has_match_write(condition: &Node<'_>) -> bool {
    matches!(condition, Node::MatchWriteNode { .. })
}
/// True when any span in `spans` (source-ordered, as recorded by `enter`)
/// lies entirely within `range`, found with a binary search instead of a
/// subtree walk.
fn any_span_within(spans: &[Span], range: Span) -> bool {
    let start = spans.partition_point(|s| s.start < range.start);
    spans[start..].iter().take_while(|s| s.start < range.end).any(|s| s.end <= range.end)
}

/// Names among `pairs` (source-ordered `(span, name)`, as recorded by
/// `enter`) whose span lies entirely within `range`.
fn names_within(pairs: &[(Span, Box<[u8]>)], range: Span) -> impl Iterator<Item = &[u8]> {
    let start = pairs.partition_point(|(span, _)| span.start < range.start);
    pairs[start..]
        .iter()
        .take_while(move |(span, _)| span.start < range.end)
        .filter(move |(span, _)| span.end <= range.end)
        .map(|(_, name)| name.as_ref())
}

/// RuboCop's `nonempty_line_count`: non-blank lines within `span`.
fn nonempty_line_count(ctx: &Context<'_>, span: Span) -> u32 {
    let first = ctx.line_col(span.start).line;
    let last = ctx.line_col(span.end.saturating_sub(1)).line;
    u32::try_from(
        (first..=last)
            .filter(|&line| !ctx.line_text(line).iter().all(u8::is_ascii_whitespace))
            .count(),
    )
    .unwrap_or(u32::MAX)
}

/// RuboCop's `first_line_comment`: a comment on `line`, unless it disables
/// this cop (or `all`) via `# rubocop:disable`/`todo`.
fn first_line_comment<'a>(ctx: &Context<'a>, line: u32) -> Option<&'a [u8]> {
    let comment = ctx.comments().iter().find(|c| c.line == line)?;
    let disables_us = ctx.directives().directives().iter().any(|d| {
        d.span.start >= comment.span.start
            && d.span.end <= comment.span.end
            && d.kind.disables()
            && d.cops.iter().any(|c| c.covers("Style/IfUnlessModifier"))
    });
    if disables_us {
        None
    } else {
        Some(ctx.text(comment.span))
    }
}

/// RuboCop's `code_after`: non-empty trailing text on `end_loc`'s line.
fn code_after<'a>(ctx: &Context<'a>, end_loc: &Location<'_>) -> Option<&'a [u8]> {
    let end_span = end_loc.span();
    let line = ctx.line_col(end_span.end.saturating_sub(1)).line;
    let line_span = ctx.line_span(line);
    let rel = usize::try_from(end_span.end.saturating_sub(line_span.start)).unwrap_or(0);
    let text = ctx.line_text(line);
    if rel >= text.len() {
        None
    } else {
        let code = &text[rel..];
        if code.is_empty() {
            None
        } else {
            Some(code)
        }
    }
}

/// RuboCop's `if_body_source`: usually the statement's raw source, except
/// for a parenthesis-less call whose last argument is a hash/keyword-hash
/// ending in an omitted value (`obj.foo bar:`), which gets parenthesized.
fn if_body_source(ctx: &Context<'_>, stmt: &Node<'_>) -> Vec<u8> {
    if let Node::CallNode { .. } = stmt {
        let call = stmt.as_call_node().expect("kind matched");
        if call.name().as_slice() != b"[]=" {
            if let Some(arguments) = call.arguments() {
                let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
                if let Some(last) = args.last() {
                    if omitted_value_in_last_hash_arg(last) {
                        let mut out = method_source(ctx, &call);
                        out.push(b'(');
                        for (i, arg) in args.iter().enumerate() {
                            if i > 0 {
                                out.extend_from_slice(b", ");
                            }
                            out.extend_from_slice(ctx.text(arg.span()));
                        }
                        out.push(b')');
                        return out;
                    }
                }
            }
        }
    }
    ctx.text(stmt.span()).to_vec()
}

/// True when `node` is a hash/keyword-hash whose last pair's value was
/// omitted (`{ foo: }` / `foo:`).
fn omitted_value_in_last_hash_arg(node: &Node<'_>) -> bool {
    let elements = match node {
        Node::KeywordHashNode { .. } => {
            node.as_keyword_hash_node().expect("kind matched").elements()
        }
        Node::HashNode { .. } => node.as_hash_node().expect("kind matched").elements(),
        _ => return false,
    };
    let Some(last) = elements.last() else { return false };
    let Node::AssocNode { .. } = &last else { return false };
    let assoc = last.as_assoc_node().expect("kind matched");
    matches!(assoc.value(), Node::ImplicitNode { .. })
}

/// RuboCop's `method_source`: the call's own source from its start through
/// its method name (or, for an implicit `.()`/`&.()` call, through the call
/// operator).
fn method_source(ctx: &Context<'_>, call: &CallNode<'_>) -> Vec<u8> {
    let start = call.location().span().start;
    let end = match call.message_loc() {
        Some(loc) => loc.span().end,
        None => call.call_operator_loc().map_or(call.location().span().end, |loc| loc.span().end),
    };
    ctx.text(Span::new(start, end)).to_vec()
}

/// The last argument of `stmt`, when `stmt` is a call with at least one
/// argument (RuboCop's `node.if_branch.last_argument if node.if_branch.send_type?`).
fn last_argument_if_call<'pr>(stmt: &Node<'pr>) -> Option<Node<'pr>> {
    let Node::CallNode { .. } = stmt else { return None };
    let call = stmt.as_call_node().expect("kind matched");
    call.arguments()?.arguments().last()
}

/// If `node` is a heredoc string (its opening delimiter starts with `<<`),
/// its opening/content/closing locations (RuboCop's `extract_heredoc_from`,
/// generalized to whichever string kind Prism used).
fn heredoc_regions<'pr>(node: &Node<'pr>) -> Option<(Location<'pr>, Span, Location<'pr>)> {
    match node {
        Node::StringNode { .. } => {
            let s = node.as_string_node().expect("kind matched");
            Some((s.opening_loc()?, s.content_loc().span(), s.closing_loc()?))
        }
        Node::InterpolatedStringNode { .. } => {
            let s = node.as_interpolated_string_node().expect("kind matched");
            let opening = s.opening_loc()?;
            let closing = s.closing_loc()?;
            let content = Span::new(opening.span().end, closing.span().start);
            Some((opening, content, closing))
        }
        _ => None,
    }
}

fn matches_allowed_pattern(patterns: &[Regex], line: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(line) else { return false };
    patterns.iter().any(|p| p.is_match(text))
}

/// Trims trailing ASCII spaces/tabs (Ruby's `String#rstrip`-ish, for the
/// directive-truncated line length check).
fn trim_end_ascii_space(bytes: &[u8]) -> &[u8] {
    let mut end = bytes.len();
    while end > 0 && matches!(bytes[end - 1], b' ' | b'\t') {
        end -= 1;
    }
    &bytes[..end]
}

/// Trims a single trailing newline (Ruby's `String#chomp`), for a
/// heredoc body/terminator's raw source.
fn trim_end_newline(bytes: &[u8]) -> &[u8] {
    if let Some(stripped) = bytes.strip_suffix(b"\n") {
        stripped.strip_suffix(b"\r").unwrap_or(stripped)
    } else {
        bytes
    }
}

/// RuboCop's `find_containing_collection`/`collection_from_ancestor`: the
/// span of the nearest enclosing array/call/hash literal for which the
/// current node (per [`Context::ancestors`]) is a direct element,
/// argument, or hash-pair value -- skipping the `StatementsNode`/
/// `ParenthesesNode` wrapper an explicit `(...)` grouping adds in Prism
/// (whitequark's single `begin_type?` node) and Prism's own
/// `ArgumentsNode` wrapper (absent from whitequark's `send` children).
fn containing_collection_span(ctx: &Context<'_>) -> Option<(NodeKind, Span)> {
    let ancestors = ctx.ancestors();
    let mut i = ancestors.len().checked_sub(1)?;
    while matches!(
        ancestors[i].kind,
        NodeKind::StatementsNode | NodeKind::ParenthesesNode | NodeKind::ArgumentsNode
    ) {
        i = i.checked_sub(1)?;
    }
    match ancestors[i].kind {
        NodeKind::ArrayNode | NodeKind::CallNode => Some((ancestors[i].kind, ancestors[i].span)),
        NodeKind::AssocNode => {
            let parent = ancestors.get(i.checked_sub(1)?)?;
            matches!(parent.kind, NodeKind::HashNode | NodeKind::KeywordHashNode)
                .then_some((parent.kind, parent.span))
        }
        _ => None,
    }
}

/// Finds the node of kind `kind` whose own span is exactly `target`,
/// descending from `node` only into children whose span contains it --
/// bounded by tree depth, not file size. Matching on both kind and span
/// (not span alone) matters because an outer wrapper (e.g. the whole
/// file's `ProgramNode`, when the target collection is its only
/// statement) can share the exact same byte range.
fn node_at_span<'pr>(node: &Node<'pr>, kind: NodeKind, target: Span) -> Option<Node<'pr>> {
    let span = node.span();
    if target.start < span.start || target.end > span.end {
        return None;
    }
    if node.kind() == kind && span == target {
        return Some(*node);
    }
    let mut found = None;
    for_each_child(node, |child| {
        if found.is_none() {
            found = node_at_span(child, kind, target);
        }
    });
    found
}

/// RuboCop's `collection.children` for the collections
/// [`containing_collection_span`] recognizes: an array's elements, a
/// call's receiver and arguments, or a hash's (or keyword hash's) pairs.
fn collection_children<'pr>(collection: &Node<'pr>) -> Vec<Node<'pr>> {
    match collection {
        Node::ArrayNode { .. } => {
            collection.as_array_node().expect("kind matched").elements().iter().collect()
        }
        Node::CallNode { .. } => {
            let call = collection.as_call_node().expect("kind matched");
            let mut children: Vec<Node<'pr>> = call.receiver().into_iter().collect();
            if let Some(arguments) = call.arguments() {
                children.extend(arguments.arguments().iter());
            }
            children
        }
        Node::HashNode { .. } => {
            collection.as_hash_node().expect("kind matched").elements().iter().collect()
        }
        Node::KeywordHashNode { .. } => {
            collection.as_keyword_hash_node().expect("kind matched").elements().iter().collect()
        }
        _ => Vec::new(),
    }
}

/// RuboCop's `unwrap_begin` plus the pair-value unwrap
/// `multiline_inside_collection?` folds into it: a hash pair's value,
/// then -- for an explicit `(...)` grouping -- its first statement
/// (matching whitequark's `begin` node exposing all of its statements as
/// direct children, of which `unwrap_begin` takes only `children.first`).
fn unwrap_collection_child(node: Node<'_>) -> Option<Node<'_>> {
    let node = match &node {
        Node::AssocNode { .. } => node.as_assoc_node().expect("kind matched").value(),
        _ => node,
    };
    match &node {
        Node::ParenthesesNode { .. } => {
            let body = node.as_parentheses_node().expect("kind matched").body()?;
            match &body {
                Node::StatementsNode { .. } => {
                    body.as_statements_node().expect("kind matched").body().iter().next()
                }
                _ => Some(body),
            }
        }
        _ => Some(node),
    }
}

/// Whether `node` is an `if`/`unless` (RuboCop's `if_type?`, which
/// whitequark gives to both), and if so: its first line, its `end`
/// keyword's line (`None` for modifier form), and whether it is a
/// ternary (no `if_keyword_loc` -- ternaries are always excluded from
/// these collection checks).
fn conditional_shape(ctx: &Context<'_>, node: &Node<'_>) -> Option<(u32, Option<u32>, bool)> {
    let (end_keyword_loc, is_ternary) = match node {
        Node::IfNode { .. } => {
            let n = node.as_if_node().expect("kind matched");
            (n.end_keyword_loc(), n.if_keyword_loc().is_none())
        }
        Node::UnlessNode { .. } => {
            (node.as_unless_node().expect("kind matched").end_keyword_loc(), false)
        }
        _ => return None,
    };
    let first_line = ctx.line_col(node.span().start).line;
    let end_line = end_keyword_loc.map(|l| ctx.line_col(l.span().start).line);
    Some((first_line, end_line, is_ternary))
}

/// RuboCop's `multiline_inside_collection?`: `node_span` (a block-form
/// `if`/`unless` being considered for the modifier-form offense, whose
/// `end` keyword is on `node_end_kw_line`) is a direct element/argument/
/// hash-pair-value of an array/call/hash literal with another
/// (non-ternary) `if`/`unless` sibling that shares a line with it --
/// converting would misplace the two on one source line.
fn multiline_inside_collection(ctx: &Context<'_>, node_span: Span, node_end_kw_line: u32) -> bool {
    let Some((kind, collection_span)) = containing_collection_span(ctx) else { return false };
    let root = ctx.parsed().root();
    let Some(collection) = node_at_span(&root, kind, collection_span) else { return false };
    let node_first_line = ctx.line_col(node_span.start).line;
    collection_children(&collection).into_iter().any(|child| {
        let Some(inner) = unwrap_collection_child(child) else { return false };
        if inner.span() == node_span {
            return false;
        }
        let Some((inner_first, inner_end, is_ternary)) = conditional_shape(ctx, &inner) else {
            return false;
        };
        !is_ternary && (inner_first == node_end_kw_line || inner_end == Some(node_first_line))
    })
}

/// RuboCop's `another_modifier_if_on_same_line?`: whether the containing
/// array/call/hash literal has another modifier-form `if`/`unless`
/// anywhere inside it (at any depth) starting on `node_first_line` --
/// autocorrecting `node_span` would then place two on one line.
fn another_modifier_if_on_same_line(
    ctx: &Context<'_>,
    node_span: Span,
    node_first_line: u32,
) -> bool {
    let Some((kind, collection_span)) = containing_collection_span(ctx) else { return false };
    let root = ctx.parsed().root();
    let Some(collection) = node_at_span(&root, kind, collection_span) else { return false };
    let mut found = false;
    each_descendant(&collection, &mut |n: &Node<'_>| {
        if found || n.span() == node_span {
            return;
        }
        if let Some((first, end_line, is_ternary)) = conditional_shape(ctx, n) {
            if !is_ternary && end_line.is_none() && first == node_first_line {
                found = true;
            }
        }
    });
    found
}

/// The assigned value of every node rubocop-ast's `assignment?` matches
/// (`equals_asgn?` -- `lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`/`casgn`/`masgn` --
/// or `shorthand_asgn?` -- `op_asgn`/`or_asgn`/`and_asgn`), in Prism terms.
/// Attribute/index setters (`a.b = x`, `a[i] = x`) are plain `send`s there
/// and already covered by the call-argument tracking.
fn assignment_value<'pr>(node: &Node<'pr>) -> Option<Node<'pr>> {
    Some(match node {
        Node::LocalVariableWriteNode { .. } => node.as_local_variable_write_node()?.value(),
        Node::LocalVariableOperatorWriteNode { .. } => {
            node.as_local_variable_operator_write_node()?.value()
        }
        Node::LocalVariableAndWriteNode { .. } => node.as_local_variable_and_write_node()?.value(),
        Node::LocalVariableOrWriteNode { .. } => node.as_local_variable_or_write_node()?.value(),
        Node::InstanceVariableWriteNode { .. } => node.as_instance_variable_write_node()?.value(),
        Node::InstanceVariableOperatorWriteNode { .. } => {
            node.as_instance_variable_operator_write_node()?.value()
        }
        Node::InstanceVariableAndWriteNode { .. } => {
            node.as_instance_variable_and_write_node()?.value()
        }
        Node::InstanceVariableOrWriteNode { .. } => {
            node.as_instance_variable_or_write_node()?.value()
        }
        Node::ClassVariableWriteNode { .. } => node.as_class_variable_write_node()?.value(),
        Node::ClassVariableOperatorWriteNode { .. } => {
            node.as_class_variable_operator_write_node()?.value()
        }
        Node::ClassVariableAndWriteNode { .. } => node.as_class_variable_and_write_node()?.value(),
        Node::ClassVariableOrWriteNode { .. } => node.as_class_variable_or_write_node()?.value(),
        Node::GlobalVariableWriteNode { .. } => node.as_global_variable_write_node()?.value(),
        Node::GlobalVariableOperatorWriteNode { .. } => {
            node.as_global_variable_operator_write_node()?.value()
        }
        Node::GlobalVariableAndWriteNode { .. } => {
            node.as_global_variable_and_write_node()?.value()
        }
        Node::GlobalVariableOrWriteNode { .. } => node.as_global_variable_or_write_node()?.value(),
        Node::ConstantWriteNode { .. } => node.as_constant_write_node()?.value(),
        Node::ConstantOperatorWriteNode { .. } => node.as_constant_operator_write_node()?.value(),
        Node::ConstantAndWriteNode { .. } => node.as_constant_and_write_node()?.value(),
        Node::ConstantOrWriteNode { .. } => node.as_constant_or_write_node()?.value(),
        Node::ConstantPathWriteNode { .. } => node.as_constant_path_write_node()?.value(),
        Node::ConstantPathOperatorWriteNode { .. } => {
            node.as_constant_path_operator_write_node()?.value()
        }
        Node::ConstantPathAndWriteNode { .. } => node.as_constant_path_and_write_node()?.value(),
        Node::ConstantPathOrWriteNode { .. } => node.as_constant_path_or_write_node()?.value(),
        Node::IndexOperatorWriteNode { .. } => node.as_index_operator_write_node()?.value(),
        Node::IndexAndWriteNode { .. } => node.as_index_and_write_node()?.value(),
        Node::IndexOrWriteNode { .. } => node.as_index_or_write_node()?.value(),
        Node::CallOperatorWriteNode { .. } => node.as_call_operator_write_node()?.value(),
        Node::CallAndWriteNode { .. } => node.as_call_and_write_node()?.value(),
        Node::CallOrWriteNode { .. } => node.as_call_or_write_node()?.value(),
        Node::MultiWriteNode { .. } => node.as_multi_write_node()?.value(),
        _ => return None,
    })
}
