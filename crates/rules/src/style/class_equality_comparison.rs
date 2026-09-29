//! `Style/ClassEqualityComparison`, ported from RuboCop's
//! `lib/rubocop/cop/style/class_equality_comparison.rb` plus the
//! `AllowedMethods`/`AllowedPattern` mixins it includes.
//!
//! # Matched shape
//!
//! Upstream's `class_comparison_candidate?` node pattern matches a `send`
//! node whose method is `==`/`equal?`/`eql?`, exactly one argument, and
//! whose receiver is either `var.class` directly, or `var.class.name`/
//! `.to_s`/`.inspect` (a `CLASS_NAME_METHODS` call on top of `var.class`).
//! Either way it captures the inner `var.class` call (for the offense
//! range's start, at its `class` selector) and the comparison's sole
//! argument. This port's [`class_comparison_candidate`] returns that same
//! pair plus whether the receiver chain went through a `CLASS_NAME_METHODS`
//! call (`outer_is_class_name_method`), since upstream's `class_name` helper
//! branches on `node.children.first.method_name` -- exactly the outer
//! receiver's own method name, `:class` in the bare case or `:name`/`:to_s`/
//! `:inspect` in the chained case -- to decide how to read the RHS.
//!
//! Only a plain `send` matches upstream's pattern (a whitequark `csend` node
//! has a different type name entirely), so both the comparison call and the
//! `.class` receiver call are required to not be safe-navigation here.
//!
//! # `class_name`
//!
//! [`class_name`] mirrors upstream's private `class_name` method:
//! - Bare `var.class == X`: `X.dstr_type?` is excluded by the caller before
//!   this runs; a plain string RHS (`var.class == 'Date'`) has no valid
//!   `instance_of?` rewrite (comparing a `Class` to a `String` is always
//!   false) and returns `None`, otherwise the RHS source is used directly.
//! - Chained `var.class.name == X` (or `.to_s`/`.inspect`): if `X` is itself
//!   a call whose receiver exists and whose own method is a
//!   `CLASS_NAME_METHODS` name (`Date.name`), that receiver's source is
//!   used; else a plain string RHS is unquoted (and `::`-qualified when
//!   nested in a `class`/`module`, unless already qualified); else a
//!   variable or method-call RHS (unknown type) returns `None`; else the RHS
//!   source is used as-is (a constant like `Model`).
//!
//! `unable_to_determine_type?` (`variable?` or `call_type?`) becomes Prism's
//! four variable-read node kinds plus `CallNode` (`send`/`csend` alike, as
//! upstream's `call_type?` does not distinguish safe navigation).
//!
//! # Ranges and autocorrection
//!
//! The offense always starts at the inner `.class` call's `message_loc`
//! (matching upstream's `receiver_node.loc.selector`) and ends at the whole
//! comparison's end, so the annotation always begins at `class` and never at
//! the outer receiver (`var`). The rule still reports an offense with an
//! empty `class_argument` when `class_name` returns `None`, but emits no fix
//! for that case, matching upstream's `add_offense` block returning early
//! (`next unless class_name`) while the offense itself is still registered.
//!
//! This cop's autocorrection is unsafe (`SafeAutoCorrect: false` in
//! `config/default.yml`): there is no guarantee the constant named by
//! `class_name` actually resolves at the rewritten call site.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG` template, with `%<class_argument>s` filled by hand.
const MSG_PREFIX: &str = "Use `instance_of?";
const MSG_SUFFIX: &str = "` instead of comparing classes.";

/// RuboCop's `CLASS_NAME_METHODS`.
fn is_class_name_method(name: &[u8]) -> bool {
    matches!(name, b"name" | b"to_s" | b"inspect")
}

/// RuboCop-ast's `variable?` (`ivar`/`gvar`/`cvar`/`lvar`) plus `call_type?`
/// (`send`/`csend` alike) -- upstream's `unable_to_determine_type?`.
fn unable_to_determine_type(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::InstanceVariableReadNode
            | NodeKind::GlobalVariableReadNode
            | NodeKind::ClassVariableReadNode
            | NodeKind::LocalVariableReadNode
            | NodeKind::CallNode
    )
}

/// A `var.class` call: `(send _ :class)`, no arguments, not safe navigation.
fn as_bare_class_call<'pr>(node: &Node<'pr>) -> Option<CallNode<'pr>> {
    let call = node.as_call_node()?;
    if call.is_safe_navigation() {
        return None;
    }
    if call.name().as_slice() == b"class" && call.arguments().is_none() {
        Some(call)
    } else {
        None
    }
}

/// RuboCop's `class_comparison_candidate?`: returns the inner `var.class`
/// call, the comparison's sole RHS argument, and whether the receiver went
/// through a `CLASS_NAME_METHODS` call on top of it.
fn class_comparison_candidate<'pr>(
    call: &CallNode<'pr>,
) -> Option<(CallNode<'pr>, Node<'pr>, bool)> {
    if call.is_safe_navigation() {
        return None;
    }
    if !matches!(call.name().as_slice(), b"==" | b"equal?" | b"eql?") {
        return None;
    }
    let arguments = call.arguments()?;
    let args = arguments.arguments();
    if args.len() != 1 {
        return None;
    }
    let class_node = args.iter().next()?;
    let receiver = call.receiver()?;

    if let Some(class_call) = as_bare_class_call(&receiver) {
        return Some((class_call, class_node, false));
    }

    let outer_call = receiver.as_call_node()?;
    if outer_call.is_safe_navigation() || !is_class_name_method(outer_call.name().as_slice()) {
        return None;
    }
    let inner_receiver = outer_call.receiver()?;
    let class_call = as_bare_class_call(&inner_receiver)?;
    Some((class_call, class_node, true))
}

/// RuboCop's `trim_string_quotes` + `require_cbase?`/`string_class_name`:
/// the string RHS's own source with every `"`/`'` byte stripped, `::`
/// prepended when nested in a `class`/`module` and not already qualified.
fn string_class_name(ctx: &Context<'_>, span: Span, in_class_or_module: bool) -> Vec<u8> {
    let mut value: Vec<u8> =
        ctx.text(span).iter().copied().filter(|&b| b != b'"' && b != b'\'').collect();
    if in_class_or_module && !value.starts_with(b"::") {
        let mut prefixed = b"::".to_vec();
        prefixed.append(&mut value);
        prefixed
    } else {
        value
    }
}

/// RuboCop's `class_name`.
fn class_name(
    ctx: &Context<'_>,
    outer_is_class_name_method: bool,
    class_node: &Node<'_>,
    in_class_or_module: bool,
) -> Option<Vec<u8>> {
    if !outer_is_class_name_method {
        // `var.class == 'Foo'` compares a `Class` to a `String` (always
        // false) and has no valid `instance_of?` rewrite.
        if class_node.as_string_node().is_some() {
            return None;
        }
        return Some(ctx.text(class_node.span()).to_vec());
    }

    if let Some(call) = class_node.as_call_node() {
        if let Some(inner_receiver) = call.receiver() {
            if is_class_name_method(call.name().as_slice()) {
                return Some(ctx.text(inner_receiver.span()).to_vec());
            }
        }
    }

    if class_node.as_string_node().is_some() {
        return Some(string_class_name(ctx, class_node.span(), in_class_or_module));
    }
    // When a variable or return value of a method is used, the type is not
    // known and cannot be suggested.
    if unable_to_determine_type(class_node) {
        return None;
    }
    Some(ctx.text(class_node.span()).to_vec())
}

/// Enforces the use of `Object#instance_of?` instead of class comparison for equality.
///
/// `==`, `equal?`, and `eql?` custom method definitions are allowed by default.
/// These are customizable with the `AllowedMethods` option.
///
/// # Safety
///
/// This cop's autocorrection is unsafe because there is no guarantee that
/// the constant `Foo` exists when autocorrecting `var.class.name == 'Foo'`
/// to `var.instance_of?(Foo)`.
///
/// # Examples
///
/// ```ruby
/// # bad
/// var.class == Date
/// var.class.equal?(Date)
/// var.class.eql?(Date)
/// var.class.name == 'Date'
///
/// # good
/// var.instance_of?(Date)
/// ```
///
/// `AllowedMethods: ['==', 'equal?', 'eql?']` (default)
///
/// ```ruby
/// # good
/// def ==(other)
///   self.class == other.class && name == other.name
/// end
/// ```
///
/// `AllowedPatterns: []` (default)
///
/// ```ruby
/// # bad
/// def eq(other)
///   self.class.eq(other.class) && name.eq(other.name)
/// end
/// ```
#[derive(Debug, Clone)]
pub struct ClassEqualityComparison {
    /// `AllowedMethods`.
    allowed_methods: Vec<String>,
    /// `AllowedPatterns`, precompiled.
    allowed_patterns: Vec<Regex>,
    /// The name of every currently active `DefNode`/`DefsNode` ancestor
    /// (Prism has one `DefNode` kind for both), innermost last -- RuboCop's
    /// `node.each_ancestor(:any_def).first`.
    def_name_stack: Vec<Span>,
}

impl ClassEqualityComparison {
    /// RuboCop's `allowed_method?(name) || matches_allowed_pattern?(name)`,
    /// applied to the nearest enclosing `def`/`defs`'s name.
    fn allowed_method_name(&self, ctx: &Context<'_>, name_span: Span) -> bool {
        let name = String::from_utf8_lossy(ctx.text(name_span));
        self.allowed_methods.iter().any(|m| m == name.as_ref())
            || self.allowed_patterns.iter().any(|pattern| pattern.is_match(&name))
    }
}

impl Rule for ClassEqualityComparison {
    const META: RuleMeta = RuleMeta {
        name: "Style/ClassEqualityComparison",
        department: Department::Style,
        summary:
            "Enforces the use of `Object#instance_of?` instead of class comparison for equality.",
        explanation: "\
`==`, `equal?`, and `eql?` custom method definitions are allowed by default. \
These are customizable with the `AllowedMethods` option.

```ruby
# bad
var.class == Date
var.class.equal?(Date)
var.class.eql?(Date)
var.class.name == 'Date'

# good
var.instance_of?(Date)
```

With `AllowedMethods: ['==', 'equal?', 'eql?']` (the default):

```ruby
# good
def ==(other)
  self.class == other.class && name == other.name
end
```

With `AllowedPatterns: []` (the default):

```ruby
# bad
def eq(other)
  self.class.eq(other.class) && name.eq(other.name)
end
```

# Safety

Autocorrection is unsafe because there is no guarantee that the constant \
named on the right-hand side actually exists when autocorrecting \
`var.class.name == 'Foo'` to `var.instance_of?(Foo)`.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode, NodeKind::DefNode],
        config: &[
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(&["==", "equal?", "eql?"]),
                allowed: &[],
                doc: "Method names within which a class comparison is never flagged.",
            },
            ConfigOption {
                name: "AllowedPatterns",
                default: ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method name regex patterns within which a class comparison is never \
                      flagged, checked the same way as `AllowedMethods`.",
            },
        ],
        blind_spots: "\
`AllowedPatterns` entries that fail to compile as a Rust regex are dropped \
(never match) rather than raising a configuration error.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let allowed_methods = options.str_list("AllowedMethods");
        let allowed_patterns = options
            .str_list("AllowedPatterns")
            .iter()
            .filter_map(|pattern| Regex::new(pattern).ok())
            .collect();
        Ok(Self { allowed_methods, allowed_patterns, def_name_stack: Vec::new() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let Some(def) = node.as_def_node() {
            self.def_name_stack.push(def.name_loc().span());
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if self.def_name_stack.last().is_some_and(|&span| self.allowed_method_name(ctx, span)) {
            return;
        }
        let Some((class_call, class_node, outer_is_class_name_method)) =
            class_comparison_candidate(&call)
        else {
            return;
        };
        if class_node.as_interpolated_string_node().is_some() {
            return;
        }

        let in_class_or_module = ctx
            .ancestors()
            .iter()
            .any(|a| matches!(a.kind, NodeKind::ClassNode | NodeKind::ModuleNode));
        let resolved = class_name(ctx, outer_is_class_name_method, &class_node, in_class_or_module);

        let selector = class_call.message_loc().expect("`.class` call always has a selector");
        let range = Span::new(selector.span().start, node.span().end);
        let mut message = String::with_capacity(MSG_PREFIX.len() + MSG_SUFFIX.len() + 16);
        message.push_str(MSG_PREFIX);
        if let Some(name) = &resolved {
            message.push('(');
            message.push_str(&String::from_utf8_lossy(name));
            message.push(')');
        }
        message.push_str(MSG_SUFFIX);

        match resolved {
            Some(name) => {
                let mut replacement = b"instance_of?(".to_vec();
                replacement.extend_from_slice(&name);
                replacement.push(b')');
                ctx.report_with_fix(
                    &Self::META,
                    range,
                    message,
                    Fix {
                        applicability: Applicability::Unsafe,
                        edits: vec![Edit::replace(range, replacement)],
                    },
                );
            }
            None => ctx.report(&Self::META, range, message),
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.as_def_node().is_some() {
            self.def_name_stack.pop();
        }
    }
}
