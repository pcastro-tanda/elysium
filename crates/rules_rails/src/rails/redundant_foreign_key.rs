//! `Rails/RedundantForeignKey`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/redundant_foreign_key.rb`.
//!
//! The default foreign key of `has_one`/`has_many`/`has_and_belongs_to_many`
//! comes from rubocop-ast's `Node#parent_module_name` ([`ParentModule`])
//! fed through `ActiveSupport`'s `String#foreign_key`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

use crate::parent_module::{self, ParentModule};

const MSG: &str = "Specifying the default value for `foreign_key` is redundant.";

/// Checks for associations where the `:foreign_key` option is redundant.
#[derive(Debug, Clone, Default)]
pub struct RedundantForeignKey {
    modules: ParentModule,
}

impl Rule for RedundantForeignKey {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RedundantForeignKey",
        department: Department::Rails,
        summary: "Checks for associations where the `:foreign_key` option is redundant.",
        explanation: "Detects cases where the `:foreign_key` option on associations is \
                      redundant.\n\n```ruby\n# bad\nclass Post\n  has_many :comments, \
                      foreign_key: 'post_id'\nend\n\nclass Comment\n  belongs_to :post, \
                      foreign_key: 'post_id'\nend\n\n# good\nclass Post\n  has_many \
                      :comments\nend\n\nclass Comment\n  belongs_to :author, foreign_key: \
                      'user_id'\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: parent_module::KINDS,
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        self.modules.enter(node);
        if let Some(call) = node.as_call_node() {
            self.check(&call, ctx);
        }
    }

    fn leave(&mut self, _node: &Node<'_>, _ctx: &mut Context<'_>) {
        self.modules.leave();
    }
}

impl RedundantForeignKey {
    fn check(&self, call: &CallNode<'_>, ctx: &mut Context<'_>) {
        let association = call.name();
        let association = association.as_slice();
        if !matches!(
            association,
            b"belongs_to" | b"has_one" | b"has_many" | b"has_and_belongs_to_many"
        ) || call.receiver().is_some()
            || call.block().is_some_and(|b| b.as_block_argument_node().is_some())
        {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let mut arguments = arguments.arguments().iter();
        let (Some(name), Some(options), None) =
            (arguments.next(), arguments.next(), arguments.next())
        else {
            return;
        };
        let Some(name) = sym_or_str(&name) else { return };
        let elements: Vec<Node<'_>> = match options.kind() {
            NodeKind::HashNode => options.as_hash_node().map(|h| h.elements().iter().collect()),
            NodeKind::KeywordHashNode => {
                options.as_keyword_hash_node().map(|h| h.elements().iter().collect())
            }
            _ => None,
        }
        .unwrap_or_default();
        let pairs: Vec<_> = elements.iter().filter_map(ruby_ast::Node::as_assoc_node).collect();
        let found = pairs.iter().find_map(|pair| {
            let key = pair.key();
            if key.as_symbol_node().is_none_or(|s| s.unescaped() != b"foreign_key") {
                return None;
            }
            Some((pair, sym_or_str(&pair.value())?))
        });
        let Some((pair, foreign_key)) = found else { return };

        let default = if association == b"belongs_to" {
            Some(format!("{name}_id"))
        } else if let Some(as_value) = find_as_option(&pairs) {
            Some(format!("{as_value}_id"))
        } else {
            self.modules.name().map(|module| foreign_key_of(&module))
        };
        if default.as_deref() != Some(foreign_key.as_str()) {
            return;
        }

        let pair_span = pair.as_node().span();
        let mut range = ctx.with_surrounding_space(pair_span, Side::Left, true, false);
        let bytes = ctx.source().bytes();
        let mut begin = range.start as usize;
        while begin > 0 && bytes[begin - 1] == b',' {
            begin -= 1;
        }
        range = Span::new(u32::try_from(begin).expect("offset exceeds u32"), range.end);
        ctx.report_with_fix(
            &Self::META,
            pair_span,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(range)] },
        );
    }
}

/// `({sym str} $_)`: the value of a symbol or plain string literal.
fn sym_or_str(node: &Node<'_>) -> Option<String> {
    let bytes = match node.kind() {
        NodeKind::SymbolNode => node.as_symbol_node()?.unescaped().to_vec(),
        NodeKind::StringNode => node.as_string_node()?.unescaped().to_vec(),
        _ => return None,
    };
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// `find_as_option`: the `as:` value when it is a literal.
fn find_as_option(pairs: &[ruby_ast::node::AssocNode<'_>]) -> Option<String> {
    pairs
        .iter()
        .find(|pair| pair.key().as_symbol_node().is_some_and(|s| s.unescaped() == b"as"))
        .and_then(|pair| sym_or_str(&pair.value()))
}

/// `ActiveSupport`'s `String#foreign_key`: `underscore(demodulize(name)) + "_id"`.
fn foreign_key_of(class_name: &str) -> String {
    let demodulized = class_name.rsplit("::").next().unwrap_or(class_name);
    let chars: Vec<char> = demodulized.chars().collect();
    let mut out = String::with_capacity(chars.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            let prev = chars[i - 1];
            let after_upper_run = (prev.is_ascii_uppercase() || prev.is_ascii_digit())
                && chars.get(i + 1).is_some_and(char::is_ascii_lowercase);
            let after_lower = prev.is_ascii_lowercase() || prev.is_ascii_digit();
            if after_upper_run || after_lower {
                out.push('_');
            }
        }
        out.push(if c == '-' { '_' } else { c });
    }
    let mut key = out.to_lowercase();
    key.push_str("_id");
    key
}
