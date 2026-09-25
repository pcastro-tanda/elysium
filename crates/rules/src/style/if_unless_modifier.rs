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
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::{char_len, Span};

/// RuboCop's `MSG_USE_MODIFIER`.
const MSG_USE_MODIFIER: &str = "Favor modifier `{keyword}` usage when having a single-line body. \
Another good alternative is the usage of control flow `&&`/`||`.";

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
            NodeKind::ClassVariableWriteNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::ConstantWriteNode,
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
for the `chained?` guard and fix parenthesization) are reconstructed from a
`StatementsNode`'s direct body list and a few known wrapping constructs
(assignment to a local/instance/class/global/constant variable, `&&`/`||`,
array elements, hash values, call receiver/arguments) rather than true
parent pointers; an `if`/`unless` that is not a direct child of one of those
(e.g. inside a multiple assignment, an index write, or a `+=`/`||=`-style
operator assignment) is treated as having no left siblings and as never
needing parentheses or being chained, which only risks false negatives.

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
trailing-word extensions.",
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
                // node as plain `=`. Unlike `LocalVariableWriteNode`, these
                // aren't added to `paren_targets`: that tracking is only for
                // the fix's parenthesization/left-siblings reconstruction,
                // already documented in `blind_spots` as accepting operator
                // assignment as a false-negative-only gap there.
                self.lvasgn_spans.push(node.span());
            }
            Node::LocalVariableTargetNode { .. } => {
                // whitequark represents each target of a multiple
                // assignment (`w, h = ...`) as its own bare `lvasgn` node
                // (no value child), so `non_eligible_condition?` sees one
                // per target; Prism groups them under `MultiWriteNode`, so
                // this is the equivalent per-target node to record.
                self.lvasgn_spans.push(node.span());
            }
            Node::InstanceVariableWriteNode { .. } => {
                let n = node.as_instance_variable_write_node().expect("kind matched");
                self.paren_targets.insert(n.value().span());
            }
            Node::ClassVariableWriteNode { .. } => {
                let n = node.as_class_variable_write_node().expect("kind matched");
                self.paren_targets.insert(n.value().span());
            }
            Node::GlobalVariableWriteNode { .. } => {
                let n = node.as_global_variable_write_node().expect("kind matched");
                self.paren_targets.insert(n.value().span());
            }
            Node::ConstantWriteNode { .. } => {
                let n = node.as_constant_write_node().expect("kind matched");
                self.paren_targets.insert(n.value().span());
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
            _ => {}
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
        if self.top_level_guard(&condition, if_node.statements().as_ref(), node.span()) {
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
        if self.top_level_guard(&condition, unless_node.statements().as_ref(), node.span()) {
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

    /// RuboCop's top-of-`on_if` guards, common to both branches: an
    /// endless-method body, or a `defined?`/pattern-matching condition that
    /// would change meaning in modifier form.
    fn top_level_guard(
        &self,
        condition: &Node<'_>,
        body: Option<&StatementsNode<'_>>,
        own_span: Span,
    ) -> bool {
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
        let message = MSG_USE_MODIFIER.replace("{keyword}", keyword);
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
