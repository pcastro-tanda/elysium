//! `Style/TrivialAccessors`, ported from RuboCop's
//! `lib/rubocop/cop/style/trivial_accessors.rb` plus the `AllowedMethods`
//! mixin it includes.
//!
//! # Node shapes
//!
//! Upstream's two node-pattern matchers (`looks_like_trivial_writer?` and
//! its inline `looks_like_trivial_reader?`) rely on whitequark's collapsed
//! single-statement `def` body: a one-line reader's body is the bare `ivar`
//! node itself, not wrapped in a `begin`. Prism always wraps a `def` body in
//! a `StatementsNode`, so this port extracts the sole statement itself
//! ([`single_statement`]) and matches its kind directly --
//! [`NodeKind::InstanceVariableReadNode`] for a reader,
//! [`NodeKind::InstanceVariableWriteNode`] (whose value is itself a bare
//! [`NodeKind::LocalVariableReadNode`]) for a writer. `(args (arg ...))`
//! becomes [`single_required_parameter`]: exactly one
//! [`NodeKind::RequiredParameterNode`] and nothing else (no optional, rest,
//! post, keyword, keyword-rest or block parameter) -- which is also why
//! `accepts splats`/`accepts blocks` are rejected purely by shape, with no
//! extra check needed.
//!
//! # `top_level_node?`
//!
//! Whitequark's `node.parent.nil?` is true only when the `def` is the
//! entire program (no enclosing anything, and no sibling top-level
//! statement -- a `begin` node would otherwise be the non-nil parent).
//! Prism always wraps the program's top-level statements in a
//! `StatementsNode` regardless of how many there are, so this port instead
//! precomputes, once per file ([`Rule::file_start`]), whether that
//! top-level `StatementsNode` holds exactly one statement
//! ([`TrivialAccessors::top_level_singleton`]); a `def` is top-level in
//! upstream's sense exactly when its own ancestors are just
//! `[ProgramNode, StatementsNode]` and that flag is set.
//!
//! # `in_module_or_instance_eval?`
//!
//! Upstream walks `node.each_ancestor(:any_block, :class, :sclass,
//! :module)`, stopping at the first `class`/`sclass` (false), the first
//! `module` (true), or a block whose owning call is `instance_eval` (true);
//! any other block is silently skipped over. [`NodeKind::BlockNode`] is
//! Prism's single kind for a plain block, a numblock (`_1`) and an itblock
//! (bare `it`) alike, so no special-casing is needed for those. Since
//! [`crate::Context::ancestors`] carries only kind and span, this port
//! keeps its own `block_is_instance_eval` stack, pushed whenever a
//! `CallNode` owning a literal block ([`owns_literal_block`]) is entered
//! (recording whether its own name is `instance_eval`) and popped on
//! leaving it -- one entry per `BlockNode` ancestor currently open, in the
//! same outermost-first order, so [`TrivialAccessors::in_module_or_instance_eval`]
//! can zip the two stacks while walking ancestors innermost-first. A `&.`
//! call is irrelevant here (RuboCop's `method?` checks the name only), so
//! safe navigation is not special-cased.
//!
//! # Autocorrection
//!
//! `autocorrect`'s `return if parent&.send_type?` (skipping `private def
//! foo; ...; end`, whose `def` is a `send` node's direct child) becomes
//! [`parent_is_call_argument`]: Prism interposes an `ArgumentsNode` between
//! a `CallNode` and its argument, so the check is on the *two* innermost
//! ancestors, not one. `autocorrect_class`'s `node.children.first.self_type?`
//! (true only for `def self.foo`, not `def obj.foo`) is
//! `DefNode::receiver` matched against [`NodeKind::SelfNode`].

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::{CallNode, DefNode};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};

/// RuboCop's default `AllowedMethods`: conversion methods that are allowed
/// to be trivial readers even though their body is a bare ivar read.
const DEFAULT_ALLOWED_METHODS: &[&str] = &[
    "to_ary",
    "to_a",
    "to_c",
    "to_enum",
    "to_h",
    "to_hash",
    "to_i",
    "to_int",
    "to_io",
    "to_open",
    "to_path",
    "to_proc",
    "to_r",
    "to_regexp",
    "to_str",
    "to_s",
    "to_sym",
];

/// Whether a trivial accessor found in the wild is a reader or a writer;
/// `RuboCop::Cop::Style::TrivialAccessors::MSG`'s `%<kind>s`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AccessorKind {
    Reader,
    Writer,
}

impl AccessorKind {
    fn word(self) -> &'static str {
        match self {
            AccessorKind::Reader => "reader",
            AccessorKind::Writer => "writer",
        }
    }
}

/// Whether a `CallNode` owns a literal block (as opposed to a `&block`
/// argument pass, which is a `BlockArgumentNode`, not a `BlockNode`, and is
/// never an `any_block_type?` ancestor upstream).
fn owns_literal_block(call: &CallNode<'_>) -> bool {
    call.block().is_some_and(|b| b.as_block_node().is_some())
}

/// A `StatementsNode` with exactly one statement, standing in for that
/// statement itself -- Prism's stand-in for whitequark's collapsed
/// single-statement body. See the module doc's "Node shapes" section.
fn single_statement(body: Option<Node<'_>>) -> Option<Node<'_>> {
    let list = body?.as_statements_node()?.body();
    (list.len() == 1).then(|| list.first()).flatten()
}

/// `!node.arguments?`: no parameters at all (an empty `()` parses with no
/// `ParametersNode`, so `parameters().is_none()` alone would suffice, but a
/// defensive all-empty check costs nothing).
fn has_no_parameters(def: &DefNode<'_>) -> bool {
    def.parameters().is_none_or(|p| {
        p.requireds().is_empty()
            && p.optionals().is_empty()
            && p.rest().is_none()
            && p.posts().is_empty()
            && p.keywords().is_empty()
            && p.keyword_rest().is_none()
            && p.block().is_none()
    })
}

/// `(args (arg ...))`: exactly one required positional parameter and
/// nothing else -- rejects `*splat`, `&block`, optional, keyword and
/// destructured (`(a, b)`) parameters alike by shape.
fn single_required_parameter(def: &DefNode<'_>) -> bool {
    def.parameters().is_some_and(|p| {
        p.requireds().len() == 1
            && p.requireds().first().is_some_and(|n| n.as_required_parameter_node().is_some())
            && p.optionals().is_empty()
            && p.rest().is_none()
            && p.posts().is_empty()
            && p.keywords().is_empty()
            && p.keyword_rest().is_none()
            && p.block().is_none()
    })
}

/// `looks_like_trivial_reader?`: no parameters, body is a bare ivar read.
/// Returns the ivar's name (with its leading `@`) for `names_match?`.
fn looks_like_trivial_reader<'pr>(def: &DefNode<'pr>) -> Option<&'pr [u8]> {
    if !has_no_parameters(def) {
        return None;
    }
    let ivar = single_statement(def.body())?.as_instance_variable_read_node()?;
    Some(ivar.name().as_slice())
}

/// `looks_like_trivial_writer?`: exactly one required parameter, body is a
/// bare ivar write whose value is a bare local variable read (necessarily
/// the parameter itself, since it is the method's only statement and thus
/// the only local in scope). Returns the ivar's name.
fn looks_like_trivial_writer<'pr>(def: &DefNode<'pr>) -> Option<&'pr [u8]> {
    if !single_required_parameter(def) {
        return None;
    }
    let write = single_statement(def.body())?.as_instance_variable_write_node()?;
    write.value().as_local_variable_read_node()?;
    Some(write.name().as_slice())
}

/// `Node#predicate_method?`.
fn is_predicate(method_name: &[u8]) -> bool {
    method_name.ends_with(b"?")
}

/// `Node#comparison_method?`'s `COMPARISON_OPERATORS` (rubocop-ast's
/// `Node::COMPARISON_OPERATORS`; `<=>` is deliberately excluded upstream
/// since it does not return a boolean).
fn is_comparison_method(name: &[u8]) -> bool {
    matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=" | b">" | b"<")
}

/// `Node#assignment_method?`.
fn is_assignment_method(method_name: &[u8]) -> bool {
    method_name.ends_with(b"=") && !is_comparison_method(method_name)
}

/// `names_match?`: the method name, minus a trailing `=`/`?`, equals the
/// ivar's name minus its leading `@`.
fn names_match(method_name: &[u8], ivar_name: &[u8]) -> bool {
    let stripped = match method_name.last() {
        Some(b'=' | b'?') => &method_name[..method_name.len() - 1],
        _ => method_name,
    };
    ivar_name.first() == Some(&b'@') && stripped == &ivar_name[1..]
}

/// `accessor`: `attr_reader :foo` / `attr_writer :foo`.
fn accessor_line(kind: AccessorKind, method_name: &[u8]) -> String {
    let base = method_name.strip_suffix(b"=").unwrap_or(method_name);
    format!("attr_{} :{}", kind.word(), String::from_utf8_lossy(base))
}

/// `autocorrect`'s `return if parent&.send_type?`: a `def` immediately
/// wrapped as a `CallNode` argument (`private def foo; ...; end`). Prism
/// interposes an `ArgumentsNode` between the call and its argument, so both
/// of the two innermost ancestors matter, not just one.
fn parent_is_call_argument(ctx: &Context<'_>) -> bool {
    let ancestors = ctx.ancestors();
    let len = ancestors.len();
    len >= 2
        && ancestors[len - 1].kind == NodeKind::ArgumentsNode
        && ancestors[len - 2].kind == NodeKind::CallNode
}

/// Boolean config knobs for [`TrivialAccessors`], grouped to keep the rule
/// struct itself under clippy's bool-field limit.
#[derive(Debug, Clone, Copy, Default)]
struct AccessorOptions {
    exact_name_match: bool,
    allow_predicates: bool,
    allow_dsl_writers: bool,
}

/// Prefer attr_* methods to trivial readers/writers.
#[derive(Debug, Clone)]
pub struct TrivialAccessors {
    options: AccessorOptions,
    ignore_class_methods: bool,
    allowed_methods: Vec<String>,
    /// One entry per currently open `BlockNode` ancestor (pushed/popped
    /// alongside the `CallNode` that owns it, outermost first): whether
    /// that block's owning call is `instance_eval`. See the module doc's
    /// "`in_module_or_instance_eval?`" section.
    block_is_instance_eval: Vec<bool>,
    /// Whether the file's single top-level `StatementsNode` (the
    /// `ProgramNode`'s direct child) holds exactly one statement --
    /// RuboCop's `node.parent.nil?`, true only when a `def` is the entire
    /// program. Computed once per file in [`Rule::file_start`].
    top_level_singleton: bool,
}

impl TrivialAccessors {
    /// `allowed_method_name?`.
    fn allowed_method_name(&self, method_name: &[u8], ivar_name: &[u8]) -> bool {
        if method_name == b"initialize"
            || self.allowed_methods.iter().any(|m| m.as_bytes() == method_name)
        {
            return true;
        }
        self.options.exact_name_match && !names_match(method_name, ivar_name)
    }

    /// `in_module_or_instance_eval?`. See the module doc.
    fn in_module_or_instance_eval(&self, ctx: &Context<'_>) -> bool {
        let mut block_idx = self.block_is_instance_eval.len();
        for info in ctx.ancestors().iter().rev() {
            match info.kind {
                NodeKind::ClassNode | NodeKind::SingletonClassNode => return false,
                NodeKind::ModuleNode => return true,
                NodeKind::BlockNode => {
                    block_idx -= 1;
                    if self.block_is_instance_eval[block_idx] {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    /// `top_level_node?`. See the module doc.
    fn is_top_level(&self, ctx: &Context<'_>) -> bool {
        let ancestors = ctx.ancestors();
        ancestors.len() == 2
            && ancestors[0].kind == NodeKind::ProgramNode
            && ancestors[1].kind == NodeKind::StatementsNode
            && self.top_level_singleton
    }

    /// `trivial_accessor_kind`: the kind actually eligible for
    /// autocorrection, which -- unlike the offense's own `kind` -- treats a
    /// DSL-style writer (method name without a trailing `=`) as
    /// uncorrectable even when `AllowDSLWriters: false` still flags it.
    fn correction_kind(kind: AccessorKind, method_name: &[u8]) -> Option<AccessorKind> {
        match kind {
            AccessorKind::Writer if is_assignment_method(method_name) => Some(AccessorKind::Writer),
            AccessorKind::Reader => Some(AccessorKind::Reader),
            AccessorKind::Writer => None,
        }
    }

    /// `autocorrect`/`autocorrect_instance`/`autocorrect_class`.
    fn build_fix(
        def: &DefNode<'_>,
        ctx: &Context<'_>,
        kind: AccessorKind,
        method_name: &[u8],
        ivar_name: &[u8],
    ) -> Option<Fix> {
        if parent_is_call_argument(ctx) {
            return None;
        }
        let corrected = Self::correction_kind(kind, method_name)?;
        if !names_match(method_name, ivar_name) || is_predicate(method_name) {
            return None;
        }
        let span = def.as_node().span();
        let replacement = match def.receiver() {
            None => accessor_line(corrected, method_name),
            Some(receiver) if receiver.as_self_node().is_some() => {
                let indent = " ".repeat(ctx.line_col(span.start).column as usize);
                format!(
                    "class << self\n{indent}  {}\n{indent}end",
                    accessor_line(corrected, method_name)
                )
            }
            Some(_) => return None,
        };
        Some(Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, replacement.into_bytes())],
        })
    }

    fn on_def(&mut self, def: &DefNode<'_>, ctx: &mut Context<'_>) {
        if self.is_top_level(ctx) {
            return;
        }
        if self.in_module_or_instance_eval(ctx) {
            return;
        }
        if self.ignore_class_methods && def.receiver().is_some() {
            return;
        }

        let method_name = def.name().as_slice();
        let kind = if let Some(ivar) = looks_like_trivial_reader(def) {
            (!(self.allowed_method_name(method_name, ivar)
                || (self.options.allow_predicates && is_predicate(method_name))))
            .then_some((AccessorKind::Reader, ivar))
        } else if let Some(ivar) = looks_like_trivial_writer(def) {
            (!(self.allowed_method_name(method_name, ivar)
                || (self.options.allow_dsl_writers && !is_assignment_method(method_name))))
            .then_some((AccessorKind::Writer, ivar))
        } else {
            None
        };
        let Some((kind, ivar)) = kind else { return };

        let word = kind.word();
        let message = format!("Use `attr_{word}` to define trivial {word} methods.");
        let span = def.def_keyword_loc().span();
        match Self::build_fix(def, ctx, kind, method_name, ivar) {
            Some(fix) => ctx.report_with_fix(&Self::META, span, message, fix),
            None => ctx.report(&Self::META, span, message),
        }
    }
}

impl Rule for TrivialAccessors {
    const META: RuleMeta = RuleMeta {
        name: "Style/TrivialAccessors",
        department: Department::Style,
        summary: "Prefer attr_* methods to trivial readers/writers.",
        explanation: "\
Looks for trivial reader/writer methods, that could have been created with
the attr_* family of functions automatically.

```ruby
# bad
def foo
  @foo
end

def bar=(val)
  @bar = val
end

def self.baz
  @baz
end

# good
attr_reader :foo
attr_writer :bar

class << self
  attr_reader :baz
end
```

`to_ary`, `to_a`, `to_c`, `to_enum`, `to_h`, `to_hash`, `to_i`, `to_int`,
`to_io`, `to_open`, `to_path`, `to_proc`, `to_r`, `to_regexp`, `to_str`,
`to_s`, and `to_sym` methods are allowed by default. These are customizable
with the `AllowedMethods` option.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode, NodeKind::CallNode],
        config: &[
            ConfigOption {
                name: "ExactNameMatch",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether the ivar's name (minus `@`) must equal the method's name (minus \
                      `=`/`?`).",
            },
            ConfigOption {
                name: "AllowPredicates",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether a trivial `foo?` reader is allowed.",
            },
            ConfigOption {
                name: "AllowDSLWriters",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether a trivial writer whose name does not end in `=` (a DSL-style \
                      setter) is allowed.",
            },
            ConfigOption {
                name: "IgnoreClassMethods",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether `def self.foo`/`def self.foo=` are exempt entirely.",
            },
            ConfigOption {
                name: "AllowedMethods",
                default: ConfigDefault::StrList(DEFAULT_ALLOWED_METHODS),
                allowed: &[],
                doc: "Method names always allowed as trivial readers/writers (`initialize` is \
                      always allowed too, unconditionally).",
            },
        ],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            options: AccessorOptions {
                exact_name_match: options.bool("ExactNameMatch"),
                allow_predicates: options.bool("AllowPredicates"),
                allow_dsl_writers: options.bool("AllowDSLWriters"),
            },
            ignore_class_methods: options.bool("IgnoreClassMethods"),
            allowed_methods: options.str_list("AllowedMethods"),
            block_is_instance_eval: Vec::new(),
            top_level_singleton: false,
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.block_is_instance_eval.clear();
        self.top_level_singleton = ctx
            .parsed()
            .root()
            .as_program_node()
            .is_some_and(|program| program.statements().body().len() == 1);
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                if owns_literal_block(&call) {
                    self.block_is_instance_eval.push(call.name().as_slice() == b"instance_eval");
                }
            }
            NodeKind::DefNode => {
                let def = node.as_def_node().expect("kind matched");
                self.on_def(&def, ctx);
            }
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if node.kind() == NodeKind::CallNode {
            let call = node.as_call_node().expect("kind matched");
            if owns_literal_block(&call) {
                self.block_is_instance_eval.pop();
            }
        }
    }
}
