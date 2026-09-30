//! `Metrics/MethodLength`, ported from RuboCop's
//! `lib/rubocop/cop/metrics/method_length.rb` plus the `CodeLength` mixin
//! (`lib/rubocop/cop/mixin/code_length.rb`) and the `AllowedMethods`/
//! `AllowedPattern` mixins it includes.
//!
//! # `define_method`
//!
//! Upstream's `on_block` (aliased `on_numblock`/`on_itblock`) fires for
//! every block whose owning call is named `define_method`, checking the
//! call's *first argument* (a literal `Symbol`/`String`) against
//! `AllowedMethods`/`AllowedPatterns`. Prism attaches a block to its owning
//! `CallNode` as a field rather than wrapping both in one node (see
//! `metrics/block_length.rs`'s doc), so this subscribes to `CallNode`
//! directly instead of walking back up from a `BlockNode`.
//!
//! Upstream's `method_name&.basic_literal? && allowed?(method_name.value)`
//! is `nil`-safe: `define_method` called with no arguments at all (or a
//! non-literal first argument, e.g. a variable) never matches
//! `AllowedMethods`/`AllowedPatterns`, so the block is still measured
//! normally.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use regex::Regex;
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};

use super::util::CodeLength;

/// Avoid methods longer than 10 lines of code.
#[derive(Debug, Clone)]
pub struct MethodLength {
    code_length: CodeLength,
    /// `AllowedMethods`, plus the deprecated `IgnoredMethods`/
    /// `ExcludedMethods` aliases merged in (see `AllowedMethods`'s own
    /// `cop_config_deprecated_values`, shared with `Metrics/BlockLength`).
    allowed_methods: Vec<String>,
    /// `AllowedPatterns`, precompiled.
    allowed_patterns: Vec<Regex>,
}

impl MethodLength {
    /// Upstream's `allowed?`: `allowed_method?(name) || matches_allowed_pattern?(name)`.
    fn is_allowed(&self, name: &[u8]) -> bool {
        if self.allowed_methods.iter().any(|m| m.as_bytes() == name) {
            return true;
        }
        let Ok(name_str) = std::str::from_utf8(name) else { return false };
        self.allowed_patterns.iter().any(|p| p.is_match(name_str))
    }
}

/// Upstream's `method_name.basic_literal?`, restricted to `Symbol`/`String`
/// (the only literal kinds `define_method`'s first argument can sensibly be):
/// its unescaped bytes, standing in for `Node#value`.
fn basic_literal_name(call: &CallNode<'_>) -> Option<Vec<u8>> {
    let first = call.arguments()?.arguments().first()?;
    if let Some(sym) = first.as_symbol_node() {
        return Some(sym.unescaped().to_vec());
    }
    first.as_string_node().map(|s| s.unescaped().to_vec())
}

impl Rule for MethodLength {
    const META: RuleMeta = RuleMeta {
        name: "Metrics/MethodLength",
        department: Department::Metrics,
        summary: "Avoid methods longer than 10 lines of code.",
        explanation: "\
Checks if the length of a method exceeds some maximum value. Comment lines \
can optionally be allowed with `CountComments`. Constructs listed in \
`CountAsOne` (`array`, `hash`, `heredoc`, `method_call`) each collapse to a \
single counted line regardless of their own size. A `define_method` block is \
measured the same way as an ordinary method definition.

```ruby
# bad
Max: 2
def m
  a = 1
  a = 2
  a = 3
end

# good
Max: 2
def m
  a = 1
  a = 2
end
```

`AllowedMethods`/`AllowedPatterns` (both default to `[]`) exempt a method, or \
a `define_method` block whose first argument is a literal `Symbol`/`String` \
naming it, by name.",
        enabled_by_default: true,
        severity: Severity::Refactor,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[NodeKind::DefNode, NodeKind::CallNode],
        config: &[
            linter::ConfigOption {
                name: "Max",
                default: linter::ConfigDefault::Int(10),
                allowed: &[],
                doc: "Maximum number of counted lines a method may have.",
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
                default: linter::ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method names that are never measured. The deprecated \
                      `IgnoredMethods`/`ExcludedMethods` aliases are merged in too.",
            },
            linter::ConfigOption {
                name: "AllowedPatterns",
                default: linter::ConfigDefault::StrList(&[]),
                allowed: &[],
                doc: "Method name regex patterns that are never measured.",
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
        match node.kind() {
            NodeKind::DefNode => {
                let def = node.as_def_node().expect("kind matched");
                if self.is_allowed(def.name().as_slice()) {
                    return;
                }
            }
            NodeKind::CallNode => {
                let call = node.as_call_node().expect("kind matched");
                if call.name().as_slice() != b"define_method" {
                    return;
                }
                if call.block().and_then(|b| b.as_block_node()).is_none() {
                    return;
                }
                if let Some(name) = basic_literal_name(&call) {
                    if self.is_allowed(&name) {
                        return;
                    }
                }
            }
            _ => return,
        }

        self.code_length.check(ctx, &Self::META, node, node.span(), "Method");
    }
}
