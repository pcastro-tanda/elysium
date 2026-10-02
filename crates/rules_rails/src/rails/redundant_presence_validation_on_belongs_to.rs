//! `Rails/RedundantPresenceValidationOnBelongsTo`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/redundant_presence_validation_on_belongs_to.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::{Side, Span};

use super::application_record::target_rails_version;

const MSG: &str = "Remove explicit presence validation for ";
/// `minimum_target_rails_version 5.0`.
const MINIMUM_TARGET_RAILS_VERSION: f64 = 5.0;
/// `NON_VALIDATION_OPTIONS`.
const NON_VALIDATION_OPTIONS: [&[u8]; 6] =
    [b"if", b"unless", b"on", b"allow_blank", b"allow_nil", b"strict"];

/// Checks for redundant presence validation on `belongs_to` association.
#[derive(Debug, Clone)]
pub struct RedundantPresenceValidationOnBelongsTo {
    supported: bool,
    /// The `StatementsNode` of a `begin ... end` without `rescue`/`ensure`:
    /// whitequark's `kwbegin` holds its statements directly, so they have no
    /// `begin` parent.
    kwbegin_body: Option<Span>,
}

impl Rule for RedundantPresenceValidationOnBelongsTo {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RedundantPresenceValidationOnBelongsTo",
        department: Department::Rails,
        summary: "Checks for redundant presence validation on `belongs_to` association.",
        explanation: "Since Rails 5.0 the default for `belongs_to` is `optional: false` unless \
                      `config.active_record.belongs_to_required_by_default` is explicitly set \
                      to `false`. The presence validator is added automatically, and explicit \
                      presence validation is redundant.\n\nThis cop's autocorrection is unsafe \
                      because it changes the default error message from \"can't be blank\" to \
                      \"must exist\".\n\n```ruby\n# bad\nbelongs_to :user\nvalidates :user, \
                      presence: true\n\n# bad\nbelongs_to :user\nvalidates :user_id, presence: \
                      true\n\n# bad\nbelongs_to :author, foreign_key: :user_id\nvalidates \
                      :user_id, presence: true\n\n# good\nbelongs_to :user\n\n# good\n\
                      belongs_to :author, foreign_key: :user_id\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::StatementsNode, NodeKind::BeginNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            supported: target_rails_version(options) >= MINIMUM_TARGET_RAILS_VERSION,
            kwbegin_body: None,
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        if let Some(begin) = node.as_begin_node() {
            if begin.begin_keyword_loc().is_some()
                && begin.rescue_clause().is_none()
                && begin.ensure_clause().is_none()
            {
                self.kwbegin_body = begin.statements().map(|s| s.as_node().span());
            }
            return;
        }
        let Some(statements) = node.as_statements_node() else { return };
        if self.kwbegin_body == Some(node.span()) {
            return;
        }
        // The `(begin <...>)` matchers need a `begin` parent: two or more
        // statements.
        let siblings: Vec<Node<'_>> = statements.body().iter().collect();
        if siblings.len() < 2 {
            return;
        }
        for statement in &siblings {
            check_validates(statement, &siblings, ctx);
        }
    }
}

/// A call's arguments, with a `&block` argument counting as the last one as
/// in whitequark; `None` for a `send` that has a literal block (it is then
/// wrapped in a `block` node).
fn send_arguments<'a>(
    node: &Node<'a>,
    name: &[u8],
) -> Option<(ruby_ast::node::CallNode<'a>, Vec<Node<'a>>)> {
    let call = node.as_call_node()?;
    if call.receiver().is_some() || call.is_safe_navigation() || call.name().as_slice() != name {
        return None;
    }
    let block = call.block();
    if block.as_ref().is_some_and(|b| b.as_block_node().is_some()) {
        return None;
    }
    let mut args: Vec<Node<'a>> =
        call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
    args.extend(block);
    Some((call, args))
}

fn hash_elements<'a>(node: &Node<'a>) -> Option<Vec<Node<'a>>> {
    if let Some(hash) = node.as_hash_node() {
        Some(hash.elements().iter().collect())
    } else {
        node.as_keyword_hash_node().map(|hash| hash.elements().iter().collect())
    }
}

/// `(pair (sym :name) _)`: the pair's value when its key is that symbol.
fn pair_value<'a>(element: &Node<'a>, name: &[u8]) -> Option<Node<'a>> {
    let pair = element.as_assoc_node()?;
    let key = pair.key();
    let key = key.as_symbol_node()?;
    (key.unescaped() == name).then(|| pair.value())
}

fn symbol_value(node: &Node<'_>) -> Option<Vec<u8>> {
    node.as_symbol_node().map(|sym| sym.unescaped().to_vec())
}

fn is_const(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

fn check_validates(statement: &Node<'_>, siblings: &[Node<'_>], ctx: &mut Context<'_>) {
    let Some((_, args)) = send_arguments(statement, b"validates") else { return };
    let Some((options, keys)) = args.split_last() else { return };
    if keys.is_empty() {
        return;
    }
    let Some(all_keys) = keys.iter().map(symbol_value).collect::<Option<Vec<_>>>() else {
        return;
    };
    let Some(elements) = hash_elements(options) else { return };

    // presence: true
    let Some(presence) = elements
        .iter()
        .find(|e| pair_value(e, b"presence").is_some_and(|value| value.as_true_node().is_some()))
    else {
        return;
    };
    // !strict: true / const, !if: _
    if elements.iter().any(|e| {
        pair_value(e, b"strict").is_some_and(|v| v.as_true_node().is_some() || is_const(&v))
            || pair_value(e, b"if").is_some()
    }) {
        return;
    }

    // Prior validations remaining once presence is removed.
    let option_keys: Vec<Node<'_>> =
        elements.iter().filter_map(|e| e.as_assoc_node().map(|p| p.key())).collect();
    let remaining_validations = option_keys
        .iter()
        .filter_map(symbol_value)
        .any(|name| name != b"presence" && !NON_VALIDATION_OPTIONS.contains(&name.as_slice()));
    if !remaining_validations && option_keys.len() > 1 {
        return;
    }

    let keys: Vec<Vec<u8>> = all_keys
        .iter()
        .filter(|key| {
            belongs_to_for(siblings, key).is_some_and(|belongs_to| !is_optional(&belongs_to))
        })
        .cloned()
        .collect();
    if keys.is_empty() {
        return;
    }

    let display_keys: Vec<String> =
        keys.iter().map(|key| format!("`{}`", String::from_utf8_lossy(key))).collect();
    let message = format!("{MSG}{}.", display_keys.join("/"));
    let span = presence.span();

    let edits = if elements.len() == 1 {
        // `presence: true` is the only option.
        if keys == all_keys {
            vec![Edit::delete(ctx.whole_lines(statement.span()))]
        } else {
            remove_keys_from_validation(ctx, &keys_of(statement), &keys)
        }
    } else if keys == all_keys {
        // remove_presence_option
        let left = ctx.with_surrounding_space(span, Side::Left, true, false);
        let start = move_left_over_commas(ctx.source().bytes(), left.start);
        vec![Edit::delete(Span::new(start, span.end))]
    } else {
        extract_validation_for_keys(ctx, statement, &keys, &elements)
    };
    ctx.report_with_fix(
        &RedundantPresenceValidationOnBelongsTo::META,
        span,
        message,
        Fix { applicability: Applicability::Unsafe, edits },
    );
}

fn keys_of<'a>(statement: &Node<'a>) -> Vec<Node<'a>> {
    let Some((_, mut args)) = send_arguments(statement, b"validates") else { return Vec::new() };
    args.pop();
    args
}

/// `remove_keys_from_validation`.
fn remove_keys_from_validation(
    ctx: &Context<'_>,
    key_nodes: &[Node<'_>],
    keys: &[Vec<u8>],
) -> Vec<Edit> {
    let source = ctx.source().bytes();
    keys.iter()
        .filter_map(|key| {
            let key_node = key_nodes.iter().find(|n| symbol_value(n).as_ref() == Some(key))?;
            let span = key_node.span();
            let mut end = span.end;
            while source.get(end as usize) == Some(&b',') {
                end += 1;
            }
            let range =
                ctx.with_surrounding_space(Span::new(span.start, end), Side::Right, true, false);
            Some(Edit::delete(range))
        })
        .collect()
}

/// `extract_validation_for_keys`.
fn extract_validation_for_keys(
    ctx: &Context<'_>,
    statement: &Node<'_>,
    keys: &[Vec<u8>],
    elements: &[Node<'_>],
) -> Vec<Edit> {
    let span = statement.span();
    let column = ctx.line_col(span.start).column as usize;
    let options_without_presence: Vec<String> = elements
        .iter()
        .filter(|e| pair_value(e, b"presence").is_none())
        .map(|e| String::from_utf8_lossy(ctx.text(e.span())).into_owned())
        .collect();
    let inspected: Vec<String> = keys.iter().map(|key| inspect_symbol(key)).collect();
    let source = format!(
        "{}validates {}, {}\n",
        " ".repeat(column),
        inspected.join(", "),
        options_without_presence.join(", ")
    );
    let mut edits = remove_keys_from_validation(ctx, &keys_of(statement), keys);
    edits.push(Edit::insert(ctx.whole_lines(span).end, source.into_bytes()));
    edits
}

fn move_left_over_commas(source: &[u8], mut pos: u32) -> u32 {
    while pos > 0 && source[pos as usize - 1] == b',' {
        pos -= 1;
    }
    pos
}

/// `Symbol#inspect`.
fn inspect_symbol(name: &[u8]) -> String {
    let text = String::from_utf8_lossy(name);
    let body = text.strip_suffix(['?', '!', '=']).unwrap_or(&text);
    let mut chars = body.chars();
    let simple = chars.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        && chars.all(|c| c.is_alphanumeric() || c == '_');
    if simple {
        format!(":{text}")
    } else {
        format!(":{text:?}")
    }
}

/// `belongs_to_for?`: the matching `belongs_to` among the siblings.
fn belongs_to_for<'a>(siblings: &[Node<'a>], key: &[u8]) -> Option<Node<'a>> {
    let key_text = String::from_utf8_lossy(key);
    if let Some(normalized) = key_text.strip_suffix("_id") {
        // `belongs_to?(key: normalized, fk: key)`.
        siblings
            .iter()
            .find(|sibling| {
                belongs_to_without_fk(sibling, normalized.as_bytes())
                    || belongs_to_with_matching_fk(sibling, key)
            })
            .copied()
    } else {
        // `any_belongs_to?`.
        siblings
            .iter()
            .find(|sibling| {
                send_arguments(sibling, b"belongs_to").is_some_and(|(_, args)| {
                    args.first().and_then(symbol_value).as_deref() == Some(key)
                })
            })
            .copied()
    }
}

/// `belongs_to_without_fk?`.
fn belongs_to_without_fk(node: &Node<'_>, key: &[u8]) -> bool {
    let Some((_, args)) = send_arguments(node, b"belongs_to") else { return false };
    if args.first().and_then(symbol_value).as_deref() != Some(key) {
        return false;
    }
    match args.as_slice() {
        [_] => true,
        [_, second, ..] if hash_elements(second).is_none() => true,
        [_, second] => hash_elements(second).is_some_and(|elements| {
            !elements.iter().any(|e| pair_value(e, b"foreign_key").is_some())
        }),
        _ => false,
    }
}

/// `belongs_to_with_a_matching_fk?`.
fn belongs_to_with_matching_fk(node: &Node<'_>, fk: &[u8]) -> bool {
    let Some((_, args)) = send_arguments(node, b"belongs_to") else { return false };
    let Some(last) = args.last() else { return false };
    hash_elements(last).is_some_and(|elements| {
        elements.iter().any(|e| {
            pair_value(e, b"foreign_key").and_then(|v| symbol_value(&v)).as_deref() == Some(fk)
        })
    })
}

/// `optional?`.
fn is_optional(node: &Node<'_>) -> bool {
    let Some((_, args)) = send_arguments(node, b"belongs_to") else { return false };
    if args.len() < 2 {
        return false;
    }
    let Some(elements) = args.last().and_then(hash_elements) else { return false };
    elements.iter().any(|e| {
        pair_value(e, b"optional").is_some_and(|v| v.as_true_node().is_some())
            || pair_value(e, b"required").is_some_and(|v| v.as_false_node().is_some())
    })
}
