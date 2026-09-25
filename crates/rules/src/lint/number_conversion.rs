//! `Lint/NumberConversion`, ported from RuboCop's
//! `lib/rubocop/cop/lint/number_conversion.rb` plus the `AllowedMethods`/
//! `AllowedPattern` mixins it includes.
//!
//! # Two independent shapes
//!
//! `to_method` matches a bare, argumentless `.to_i`/`.to_f`/`.to_c`/`.to_r`
//! dispatch (`handle_conversion_method`/[`NumberConversion::handle_conversion_method`]);
//! `to_method_symbol` matches the same method names passed as a `Symbol`
//! argument or `&:sym` block-pass to some other call, e.g. `.send(:to_i)` or
//! `.map(&:to_f)` (`handle_as_symbol`/[`NumberConversion::handle_as_symbol`]).
//! Prism folds a literal `&block`/`&:sym` pass into `CallNode::block` (a
//! `BlockArgumentNode`), unlike whitequark's `send` node, which keeps it as
//! an ordinary trailing `block_pass` argument; [`NumberConversion::handle_as_symbol`]
//! recovers upstream's single node-pattern match (and its `node.arguments.one?`
//! guard) by checking `CallNode::arguments` first and falling back to
//! `CallNode::block` only when there are no real arguments at all.
//!
//! Upstream's `to_method_symbol` block also binds a `receiver` parameter
//! that is, per the pattern's own child order (`(call _ $_ ...)`), actually
//! the *outer* call's method name (e.g. `:send`, `:try`, `:map`) -- never
//! `nil` -- so `next if receiver.nil?` never fires; only the trailing
//! `!node.arguments.one?` guard does anything observable, and only that one
//! is ported here.
//!
//! # `IgnoredNode`/`ignore_node`
//!
//! [`NumberConversion::handle_conversion_method`] still reports every
//! offense but only attaches a [`Fix`] to the first one covering a given
//! span: a `case foo.to_f ... end.to_i` registers two independent offenses
//! (the condition's `.to_f` is not itself a `to_i`/`to_f`/... receiver, so it
//! is not exempted, while `var.to_i.to_f`'s outer `.to_f` *is* exempted
//! because its receiver is itself a conversion-method call -- see
//! `conversion_method?` in [`NumberConversion::allow_receiver`]) but
//! RuboCop's `Corrector` can only apply one of two overlapping edits, so the
//! cop's own `ignore_node`/`part_of_ignored_node?` suppresses the nested
//! correction. [`NumberConversion::corrected_spans`] mirrors that: every
//! span a fix was actually emitted for, checked before emitting a nested
//! one.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::CallNode;
use ruby_ast::{ext, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `CONVERSION_METHOD_CLASS_MAPPING`, `%<number_object>s` filled
/// in with `object` (the receiver's, or the block param's, source text).
fn replacement(method: &[u8], object: &str) -> Option<String> {
    match method {
        b"to_i" => Some(format!("Integer({object}, 10)")),
        b"to_f" => Some(format!("Float({object})")),
        b"to_c" => Some(format!("Complex({object})")),
        b"to_r" => Some(format!("Rational({object})")),
        _ => None,
    }
}

/// RuboCop's `CONVERSION_METHODS`: `to_i`/`to_f`/`to_c`/`to_r` plus the
/// `Kernel` methods that safely replace them, checked by `conversion_method?`
/// so a receiver that is itself already a safe (or unsafe-but-nested)
/// conversion is not flagged a second time.
fn is_conversion_method(name: &[u8]) -> bool {
    matches!(
        name,
        b"Integer" | b"Float" | b"Complex" | b"Rational" | b"to_i" | b"to_f" | b"to_c" | b"to_r"
    )
}

/// rubocop-ast's `Node#numeric_type?`: integer, float, rational, and
/// complex literals.
fn is_numeric_literal(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::IntegerNode
            | NodeKind::FloatNode
            | NodeKind::RationalNode
            | NodeKind::ImaginaryNode
    )
}

/// Warns the usage of unsafe number conversions.
#[derive(Debug, Clone)]
pub struct NumberConversion {
    /// `AllowedClasses` (plus its pre-1.88 spelling `IgnoredClasses`):
    /// receivers whose top-level constant receiver (`Time`, `DateTime`, ...)
    /// is never flagged, however many method calls sit between it and the
    /// `to_i`/etc. call.
    allowed_classes: Vec<String>,
    /// `AllowedMethods`.
    allowed_methods: Vec<String>,
    /// `AllowedPatterns`.
    allowed_patterns: Vec<Regex>,
    /// RuboCop's `@ignored_nodes` (`IgnoredNode` mixin): spans a [`Fix`] was
    /// already emitted for, cleared every file. See the module doc.
    corrected_spans: Vec<Span>,
}

impl NumberConversion {
    /// RuboCop's `allowed_method_name?`.
    fn allowed_method_name(&self, name: &[u8]) -> bool {
        let name = String::from_utf8_lossy(name);
        self.allowed_methods.iter().any(|m| m == name.as_ref())
            || self.allowed_patterns.iter().any(|pattern| pattern.is_match(&name))
    }

    /// RuboCop's `allow_receiver?`.
    fn allow_receiver(&self, receiver: &Node<'_>) -> bool {
        if is_numeric_literal(receiver.kind()) {
            return true;
        }
        // RuboCop >= 1.88: `receiver.call_type?`, so a `&.` receiver call
        // qualifies too (`10&.minutes.to_i` with `AllowedMethods: [minutes]`).
        if let Some(call) = receiver.as_call_node() {
            let name = call.name();
            let bytes = name.as_slice();
            if is_conversion_method(bytes) || self.allowed_method_name(bytes) {
                return true;
            }
        }
        let top = ext::top_receiver(*receiver);
        match top.kind() {
            NodeKind::ConstantReadNode | NodeKind::ConstantPathNode => {
                ext::const_name(&top).is_some_and(|name| self.allowed_classes.contains(&name))
            }
            _ => false,
        }
    }

    /// RuboCop's `IgnoredNode#part_of_ignored_node?`: `span` falls entirely
    /// inside a previously-corrected node.
    fn part_of_corrected(&self, span: Span) -> bool {
        self.corrected_spans.iter().any(|s| s.start <= span.start && s.end >= span.end)
    }

    /// RuboCop's `handle_conversion_method`: a receiverful, argumentless
    /// `.to_i`/`.to_f`/`.to_c`/`.to_r` dispatch (`to_method`'s node pattern
    /// has no trailing `...`, so real arguments -- e.g. `'10'.to_i(16)` --
    /// never match it).
    fn handle_conversion_method(&mut self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let method = call.name();
        let method_bytes = method.as_slice();
        if !matches!(method_bytes, b"to_i" | b"to_f" | b"to_c" | b"to_r")
            || call.arguments().is_some()
        {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if self.allow_receiver(&receiver) {
            return;
        }

        let receiver_src = String::from_utf8_lossy(ctx.text(receiver.span())).into_owned();
        let method_str = String::from_utf8_lossy(method_bytes).into_owned();
        let corrected = replacement(method_bytes, &receiver_src).expect("matched above");
        // RuboCop >= 1.88 (#15252): the message spells the call's own
        // safe-navigation operator, and nothing whose receiver chain contains
        // a `&.` anywhere (`safe_navigation?`: the node or any descendant) is
        // autocorrected, because `Integer(x, 10)` would raise where `x&.to_i`
        // returned nil.
        let own_safe_navigation = call.is_safe_navigation();
        let mut chain_safe_navigation =
            receiver.as_call_node().is_some_and(|c| c.is_safe_navigation());
        ruby_ast::each_descendant(&receiver, &mut |n| {
            chain_safe_navigation |= n.as_call_node().is_some_and(|c| c.is_safe_navigation());
        });
        let safe_navigation = own_safe_navigation || chain_safe_navigation;
        let operator = if own_safe_navigation { "&." } else { "." };
        let message = format!(
            "Replace unsafe number conversion with number class parsing, instead of using \
             `{receiver_src}{operator}{method_str}`, use stricter `{corrected}`."
        );
        let span = call.as_node().span();
        if safe_navigation || self.part_of_corrected(span) {
            ctx.report(&Self::META, span, message);
            return;
        }
        self.corrected_spans.push(span);
        let fix = Fix {
            applicability: Applicability::Unsafe,
            edits: vec![Edit::replace(span, corrected.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }

    /// RuboCop's `handle_as_symbol`. See the module doc for the
    /// `arguments()`/`block()` split this needs that whitequark does not.
    fn handle_as_symbol(call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let args = call.arguments().map(|a| a.arguments());
        let (captured_span, method_bytes, total_args) =
            if let Some(first) = args.as_ref().and_then(ruby_ast::node::NodeList::first) {
                let Some(sym) = first.as_symbol_node() else { return };
                (first.span(), sym.unescaped().to_vec(), args.as_ref().expect("checked").len())
            } else if let Some(block) = call.block() {
                let Some(block_pass) = block.as_block_argument_node() else { return };
                let Some(expr) = block_pass.expression() else { return };
                let Some(sym) = expr.as_symbol_node() else { return };
                (block.span(), sym.unescaped().to_vec(), 1)
            } else {
                return;
            };
        if !matches!(method_bytes.as_slice(), b"to_i" | b"to_f" | b"to_c" | b"to_r")
            || total_args != 1
        {
            return;
        }

        let corrected = replacement(&method_bytes, "i").expect("matched above");
        let block_form = format!("{{ |i| {corrected} }}");
        let current = String::from_utf8_lossy(ctx.text(captured_span)).into_owned();
        let message = format!(
            "Replace unsafe number conversion with number class parsing, instead of using \
             `{current}`, use stricter `{block_form}`."
        );

        let span = call.as_node().span();
        let mut edits = Vec::new();
        if let (Some(open), Some(close)) = (call.opening_loc(), call.closing_loc()) {
            edits.push(Edit::replace(open.span(), b" ".to_vec()));
            edits.push(Edit::delete(close.span()));
        }
        edits.push(Edit::replace(captured_span, block_form.into_bytes()));
        let fix = Fix { applicability: Applicability::Unsafe, edits };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }
}

impl Rule for NumberConversion {
    const META: RuleMeta = RuleMeta {
        name: "Lint/NumberConversion",
        department: Department::Lint,
        summary: "Warns the usage of unsafe number conversions.",
        explanation: "\
Warns the usage of unsafe number conversions. Unsafe number conversion can
cause unexpected error if auto type conversion fails. This cop prefers
parsing with a number class instead.

Conversion with `Integer`, `Float`, etc. will raise an `ArgumentError` if
given input that is not numeric (e.g. an empty string), whereas `to_i`, etc.
will try to convert regardless of input (`''.to_i => 0`). As such, this cop
is disabled by default because it's not necessarily always correct to raise
if a value is not numeric.

NOTE: Some values cannot be converted properly using one of the `Kernel`
methods (for instance, `Time` and `DateTime` values are allowed by this cop
by default). Similarly, Rails' duration methods do not work well with
`Integer()` and can be allowed with `AllowedMethods`. By default, there are
no methods allowed.

```ruby
# bad
'10'.to_i
'10.2'.to_f
'10'.to_c
'1/3'.to_r
['1', '2', '3'].map(&:to_i)
foo.try(:to_f)
bar.send(:to_c)

# good
Integer('10', 10)
Float('10.2')
Complex('10')
Rational('1/3')
['1', '2', '3'].map { |i| Integer(i, 10) }
foo.try { |i| Float(i) }
bar.send { |i| Complex(i) }
```

With `AllowedMethods: [minutes]` (default: `[]`):

```ruby
# good
10.minutes.to_i
```

With `AllowedPatterns: ['min*']` (default: `[]`):

```ruby
# good
10.minutes.to_i
```

With `AllowedClasses: [Time, DateTime]` (the default; `IgnoredClasses` is the
pre-1.88 spelling and is still honoured):

```ruby
# good
Time.now.to_datetime.to_i
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method names on a `to_i`/etc. receiver that are always allowed.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method name regex patterns on a `to_i`/etc. receiver that are always \
                      allowed.",
            },
            ConfigOption {
                name: "AllowedClasses",
                default: ConfigDefault::StrList(&["Time", "DateTime"]),
                allowed: &[],
                doc: "Top-level constant receivers never flagged.",
            },
            ConfigOption {
                name: "IgnoredClasses",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Deprecated pre-1.88 spelling of `AllowedClasses`; both lists apply.",
            },
        ],
        blind_spots: "\
Upstream's `to_method_symbol` node pattern binds a `receiver` block parameter
that is actually the *outer* call's own method name (never `nil`, given the
pattern's child order), so its `next if receiver.nil?` guard never fires in
practice; this port therefore only implements the `node.arguments.one?`
guard that does. `AllowedMethods`/`AllowedPatterns` are matched against the
receiver call's own method name, not its full source text.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let mut allowed_classes = options.str_list("AllowedClasses");
        allowed_classes.extend(options.str_list("IgnoredClasses"));
        let allowed_methods = options.str_list("AllowedMethods");
        let allowed_patterns = options
            .str_list("AllowedPatterns")
            .iter()
            .filter_map(|pattern| Regex::new(pattern).ok())
            .collect();
        Ok(Self { allowed_classes, allowed_methods, allowed_patterns, corrected_spans: Vec::new() })
    }

    fn file_start(&mut self, _ctx: &mut Context<'_>) {
        self.corrected_spans.clear();
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        self.handle_conversion_method(&call, ctx);
        Self::handle_as_symbol(&call, ctx);
    }
}
