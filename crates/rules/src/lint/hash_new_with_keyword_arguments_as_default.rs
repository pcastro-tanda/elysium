//! `Lint/HashNewWithKeywordArgumentsAsDefault`, ported from RuboCop's
//! `lib/rubocop/cop/lint/hash_new_with_keyword_arguments_as_default.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG`.
const MSG: &str = "Use a hash literal instead of keyword arguments.";

/// Checks for the deprecated use of keyword arguments for hash default in `Hash.new`.
#[derive(Debug, Clone)]
pub struct HashNewWithKeywordArgumentsAsDefault;

impl Rule for HashNewWithKeywordArgumentsAsDefault {
    const META: RuleMeta = RuleMeta {
        name: "Lint/HashNewWithKeywordArgumentsAsDefault",
        department: Department::Lint,
        summary:
            "Checks for the deprecated use of keyword arguments for hash default in `Hash.new`.",
        explanation: "\
Checks for the deprecated use of keyword arguments as a default in `Hash.new`.

This usage raises a warning in Ruby 3.3 and results in an error in Ruby 3.4.
In Ruby 3.4, keyword arguments will instead be used to change the behavior of
a hash. For example, the capacity option can be passed to create a hash with
a certain size if you know it in advance, for better performance.

```ruby
# bad
Hash.new(key: :value)

# good
Hash.new({key: :value})
```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.name().as_slice() != b"new" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };

        let const_name = if let Some(c) = receiver.as_constant_read_node() {
            c.name().as_slice()
        } else if let Some(path) = receiver.as_constant_path_node() {
            if path.parent().is_some() {
                return;
            }
            let Some(name) = path.name() else { return };
            name.as_slice()
        } else {
            return;
        };
        if const_name != b"Hash" {
            return;
        }

        // RuboCop's node pattern `(send recv :new $[hash !braces?])`: the
        // call must have exactly one argument, and it must be a braceless
        // (keyword) hash -- an explicit-brace `Hash.new({...})` is a
        // `HashNode`, not a `KeywordHashNode`, and does not match.
        let Some(arguments) = call.arguments() else { return };
        let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let [arg] = args.as_slice() else { return };
        let Some(hash) = arg.as_keyword_hash_node() else { return };

        // RuboCop's `if first_argument.pairs.one? ... return if key is :capacity`.
        let elements: Vec<Node<'_>> = hash.elements().iter().collect();
        if let [element] = elements.as_slice() {
            if let Some(assoc) = element.as_assoc_node() {
                if let Some(sym) = assoc.key().as_symbol_node() {
                    if sym.unescaped() == b"capacity" {
                        return;
                    }
                }
            }
        }

        let span = arg.span();
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![
                Edit::insert(span.start, b"{".to_vec()),
                Edit::insert(span.end, b"}".to_vec()),
            ],
        };
        ctx.report_with_fix(&Self::META, span, MSG, fix);
    }
}
