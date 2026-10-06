//! `Rails/HttpStatusNameConsistency`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/http_status_name_consistency.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};

/// `requires_gem 'rack', '>= 3.1.0'`.
const RACK_REQUIREMENT: &[&str] = &[">= 3.1.0"];

/// `PREFERRED_STATUSES`.
const PREFERRED_STATUSES: &[(&[u8], &str)] = &[
    (b"unprocessable_entity", "unprocessable_content"),
    (b"payload_too_large", "content_too_large"),
];

/// Enforces consistency by using the current HTTP status names.
#[derive(Debug, Clone)]
pub struct HttpStatusNameConsistency {
    /// Whether the locked `rack` satisfies `requires_gem`; RuboCop does not
    /// run the cop at all otherwise.
    supported: bool,
}

impl Rule for HttpStatusNameConsistency {
    const META: RuleMeta = RuleMeta {
        name: "Rails/HttpStatusNameConsistency",
        department: Department::Rails,
        summary: "Enforces consistency by using the current HTTP status names.",
        explanation: "Enforces consistency by using the current HTTP status names.\n\n\
                      ```ruby\n# bad\nrender json: { error: \"Invalid data\" }, status: \
                      :unprocessable_entity\nhead :payload_too_large\n\n# good\nrender json: \
                      { error: \"Invalid data\" }, status: :unprocessable_content\nhead \
                      :content_too_large\n```",
        enabled_by_default: false,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { supported: options.requires_gem("rack", RACK_REQUIREMENT) })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if call.receiver().is_some() {
            return;
        }
        let name = call.name();
        let name = name.as_slice();
        if !matches!(
            name,
            b"render" | b"redirect_to" | b"head" | b"assert_response" | b"assert_redirected_to"
        ) {
            return;
        }
        let mut args: Vec<Node<'_>> =
            call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        // A `&block` argument is one more `send` argument in whitequark.
        let block_pass = call.block().is_some_and(|b| b.as_block_argument_node().is_some());
        if block_pass {
            if let Some(block) = call.block() {
                args.push(block);
            }
        }

        let status_node = match name {
            b"render" | b"redirect_to" => match args.as_slice() {
                [_, hash] | [hash] if is_hash(hash) => Some(hash),
                _ => None,
            },
            b"head" | b"assert_response" => args.first(),
            _ => match args.as_slice() {
                [_, hash, ..] | [hash, ..] if is_hash(hash) => Some(hash),
                _ => None,
            },
        };
        let Some(status_node) = status_node else { return };

        if is_hash(status_node) {
            // `status_hash_value`: the value of the first `status:` pair.
            if let Some(value) = status_hash_value(status_node) {
                check_status_name_consistency(&value, ctx);
            }
        } else {
            check_status_name_consistency(status_node, ctx);
        }
    }
}

fn is_hash(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::HashNode | NodeKind::KeywordHashNode)
}

/// `(hash <(pair (sym :status) $_) ...>)`.
fn status_hash_value<'pr>(hash: &Node<'pr>) -> Option<Node<'pr>> {
    let elements = match hash.as_hash_node() {
        Some(hash) => hash.elements(),
        None => hash.as_keyword_hash_node()?.elements(),
    };
    elements.iter().find_map(|element| {
        let pair = element.as_assoc_node()?;
        let key = pair.key();
        let key = key.as_symbol_node()?;
        (key.unescaped() == b"status").then(|| pair.value())
    })
}

/// Reports every preferred-status symbol in `node` (the node itself, else
/// its descendants).
fn check_status_name_consistency(node: &Node<'_>, ctx: &mut Context<'_>) {
    check_node(node, ctx);
    each_descendant(node, &mut |child| check_node(child, ctx));
}

fn check_node(node: &Node<'_>, ctx: &mut Context<'_>) {
    let Some(symbol) = node.as_symbol_node() else { return };
    let current = symbol.unescaped();
    let Some((_, preferred)) = PREFERRED_STATUSES.iter().find(|(name, _)| *name == current) else {
        return;
    };
    let span = node.span();
    let message = format!("Prefer `:{preferred}` over `:{}`.", String::from_utf8_lossy(current));
    ctx.report_with_fix(
        &HttpStatusNameConsistency::META,
        span,
        message,
        Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, format!(":{preferred}").into_bytes())],
        },
    );
}
