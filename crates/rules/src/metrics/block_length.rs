//! `Metrics/BlockLength`, ported from RuboCop's
//! `lib/rubocop/cop/metrics/block_length.rb` plus the `CodeLength` mixin
//! (`lib/rubocop/cop/mixin/code_length.rb`) and the `AllowedMethods`/
//! `AllowedPattern` mixins it includes.
//!
//! # Prism shape
//!
//! Whitequark gives every block -- an ordinary `recv.method(args) { ... }`,
//! a `super do ... end`, and a stabby lambda `->() do ... end` alike -- one
//! `block`/`numblock`/`itblock` node wrapping the call/`super`/`(send nil
//! :lambda)` it belongs to. Prism instead attaches the block to the owning
//! `CallNode`/`SuperNode`/`ForwardingSuperNode` as a field (and gives a
//! stabby lambda its own distinct `LambdaNode`, with no separate "call"
//! part at all). Rather than subscribe to `BlockNode` and walk back up to
//! its owner (which the engine's ancestor stack cannot do -- it only tracks
//! kind/span, not node identity), this subscribes to all four owning kinds
//! directly. A `CallNode`/`SuperNode`/`ForwardingSuperNode`'s own span
//! already extends through its attached block (see
//! `ruby_ast::ext::call_span_excluding_block`'s doc, which applies equally
//! to `super`), and a `LambdaNode`'s span is the whole `->...end`/`->{...}`
//! expression, so each is reported and measured as one unit, matching
//! upstream's `block_node` reporting the whole construct.
//!
//! `super`'s implicit method name/receiver (`node.method_name`/
//! `node.receiver` are always `:super`/`nil`, rubocop-ast's `SuperNode`
//! destructuring into `[nil, :super, *args]`) and a stabby lambda's
//! (`:lambda`/`nil`, `LambdaNode#method_name`/`#receiver`) are reproduced
//! literally so `AllowedMethods`/`AllowedPatterns`/`method_receiver_excluded`
//! still apply to them exactly as upstream's shared `on_block` would.
//!
//! # `AllowedMethods`' deprecated aliases
//!
//! Upstream's `AllowedMethods` mixin merges the deprecated `IgnoredMethods`/
//! `ExcludedMethods` config keys into `allowed_methods` unconditionally when
//! loaded from YAML (the alternate branch only fires for an in-process
//! `Regexp` object, which a real config file can never produce), despite the
//! CLI warning that they are obsolete parameters; both are merged in here
//! too, matched exactly as `AllowedMethods` entries (see
//! `method_receiver_excluded`).
//!
//! # `method_receiver_excluded?`
//!
//! Upstream's `method_receiver_excluded?` re-checks every `AllowedMethods`
//! entry a second, receiver-aware way: an entry containing a literal `.`
//! splits into `receiver.method` and matches only a block whose owning
//! call's receiver source (whitespace stripped) equals `receiver` *and*
//! whose method name equals `method`; an entry without a `.` matches by
//! method name alone (the receiver side becomes tautological -- see
//! `method_receiver_excluded`'s doc). This subsumes the plain
//! `allowed_method?` check upstream also runs first, so only this one
//! comparison is ported.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use regex::Regex;
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

use super::util::CodeLength;

/// Avoid long blocks with many lines.
#[derive(Debug, Clone)]
pub struct BlockLength {
    code_length: CodeLength,
    /// `AllowedMethods`, plus the deprecated `IgnoredMethods`/
    /// `ExcludedMethods` aliases merged in.
    allowed_methods: Vec<String>,
    /// `AllowedPatterns`, precompiled.
    allowed_patterns: Vec<Regex>,
}

impl BlockLength {
    /// Upstream's `method_receiver_excluded?`: `config` is one
    /// `AllowedMethods` entry. A `receiver.method` entry matches a block
    /// call whose own (whitespace-stripped) receiver source equals
    /// `receiver` and whose method name equals `method`; a plain `method`
    /// entry (no `.`) matches by method name alone, regardless of receiver
    /// (upstream reassigns `receiver = node_receiver` before comparing,
    /// making that half of the check tautological).
    fn method_receiver_excluded(&self, method_name: &[u8], receiver_text: Option<&[u8]>) -> bool {
        self.allowed_methods.iter().any(|config| {
            let bytes = config.as_bytes();
            match bytes.iter().position(|&b| b == b'.') {
                Some(dot) => {
                    let (receiver, method) = (&bytes[..dot], &bytes[dot + 1..]);
                    method == method_name && receiver_text == Some(receiver)
                }
                None => bytes == method_name,
            }
        })
    }
}

/// `rubocop-ast`'s `Node#class_constructor?`, restricted to the `any_block`
/// alternative (`call` is only ever reached here after confirming it has an
/// attached literal block): an attached-block call to `.new` on a bare or
/// top-level-qualified `Class`/`Module`/`Struct`, or to `.define` on `Data`.
fn is_class_constructor(call: &CallNode<'_>) -> bool {
    let Some(receiver) = call.receiver() else { return false };
    if !is_bare_or_toplevel_const(&receiver) {
        return false;
    }
    let Some(name) = const_name(&receiver) else { return false };
    match call.name().as_slice() {
        b"new" => matches!(name.as_str(), "Class" | "Module" | "Struct"),
        b"define" => name == "Data",
        _ => false,
    }
}

/// Upstream's `node.receiver&.source&.gsub(/\s+/, '')`: `text` with every
/// whitespace run removed entirely (not collapsed).
fn strip_whitespace(text: &[u8]) -> Vec<u8> {
    text.iter().copied().filter(|b| !b.is_ascii_whitespace()).collect()
}

/// The implicit method name, receiver source, and `class_constructor?`
/// status of `node`'s attached block, for whichever of the four subscribed
/// kinds it is -- `None` when there is no literal block attached (a `&blk`
/// pass is a `BlockArgumentNode` and fails `as_block_node`, so it is never
/// mistaken for one).
fn block_owner_facts(
    ctx: &Context<'_>,
    node: &Node<'_>,
) -> Option<(Vec<u8>, Option<Vec<u8>>, bool)> {
    match node.kind() {
        NodeKind::CallNode => {
            let call = node.as_call_node()?;
            call.block()?.as_block_node()?;
            let receiver = call.receiver().map(|r| strip_whitespace(ctx.text(r.span())));
            Some((call.name().as_slice().to_vec(), receiver, is_class_constructor(&call)))
        }
        NodeKind::SuperNode => {
            node.as_super_node()?.block()?.as_block_node()?;
            Some((b"super".to_vec(), None, false))
        }
        NodeKind::ForwardingSuperNode => {
            node.as_forwarding_super_node()?.block()?;
            Some((b"super".to_vec(), None, false))
        }
        NodeKind::LambdaNode => {
            node.as_lambda_node()?;
            Some((b"lambda".to_vec(), None, false))
        }
        _ => None,
    }
}

impl Rule for BlockLength {
    const META: RuleMeta = RuleMeta {
        name: "Metrics/BlockLength",
        department: Department::Metrics,
        summary: "Avoid long blocks with many lines.",
        explanation: "\
Checks if the length of a block exceeds some maximum value. Comment lines can \
optionally be ignored with `CountComments`. Constructs listed in `CountAsOne` \
(`array`, `hash`, `heredoc`, `method_call`) each collapse to a single counted \
line regardless of their own size. This cop does not apply to `Struct.new`/\
`Class.new`/`Module.new`/`Data.define` blocks.

```ruby
# bad
Max: 2
something do
  a = 1
  a = 2
  a = 3
end

# good
Max: 2
something do
  a = 1
  a = 2
end
```

`AllowedMethods` (default: `[refine]`) exempts a block by its owning call's \
method name; an entry containing a literal `.` (e.g. `Foo.bar`) additionally \
requires the call's receiver source (whitespace stripped) to equal the part \
before the `.`. `AllowedPatterns` matches the same method name against a \
list of regexps.",
        enabled_by_default: true,
        severity: Severity::Refactor,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::CallNode,
            NodeKind::SuperNode,
            NodeKind::ForwardingSuperNode,
            NodeKind::LambdaNode,
        ],
        config: &[
            linter::ConfigOption {
                name: "Max",
                default: linter::ConfigDefault::Int(25),
                allowed: &[],
                doc: "Maximum number of counted lines a block may have.",
            },
            linter::ConfigOption {
                name: "CountComments",
                default: linter::ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Whether full-line comments count towards the total.",
            },
            linter::ConfigOption {
                name: "CountAsOne",
                default: linter::ConfigDefault::StrList(&[]),
                allowed: &["array", "hash", "heredoc", "method_call"],
                doc: "Constructs that count as a single line regardless of their own size.",
            },
            linter::ConfigOption {
                name: "AllowedMethods",
                default: linter::ConfigDefault::StrList(&["refine"]),
                allowed: &[],
                doc: "Method (optionally `receiver.method`) names owning a block that is \
                      never measured. The deprecated `IgnoredMethods`/`ExcludedMethods` \
                      aliases are merged in too.",
            },
            linter::ConfigOption {
                name: "AllowedPatterns",
                default: linter::ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method name regex patterns owning a block that is never measured.",
            },
        ],
        blind_spots: "\
`AllowedPatterns` entries that fail to compile as a Rust regex are dropped \
(never match) rather than raising a configuration error.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let mut allowed_methods = options.str_list("AllowedMethods");
        allowed_methods.extend(options.str_list("IgnoredMethods"));
        allowed_methods.extend(options.str_list("ExcludedMethods"));
        let allowed_patterns =
            options.str_list("AllowedPatterns").iter().filter_map(|p| Regex::new(p).ok()).collect();
        Ok(Self {
            code_length: CodeLength::from_options(options),
            allowed_methods,
            allowed_patterns,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some((name, receiver_text, is_ctor)) = block_owner_facts(ctx, node) else { return };
        if is_ctor {
            return;
        }
        if let Ok(name_str) = std::str::from_utf8(&name) {
            if self.allowed_patterns.iter().any(|p| p.is_match(name_str)) {
                return;
            }
        }
        if self.method_receiver_excluded(&name, receiver_text.as_deref()) {
            return;
        }

        self.code_length.check(ctx, &Self::META, node, node.span(), "Block");
    }
}
