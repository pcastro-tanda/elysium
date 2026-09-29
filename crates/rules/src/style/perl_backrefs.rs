//! `Style/PerlBackrefs`, ported from RuboCop's
//! `lib/rubocop/cop/style/perl_backrefs.rb`.
//!
//! Upstream's `on_back_ref`/`on_gvar`/`on_nth_ref` become Prism's
//! [`NodeKind::BackReferenceReadNode`] (`$~`, `$&`, `` $` ``, `$'`, `$+`),
//! [`NodeKind::GlobalVariableReadNode`] (the 'English' library aliases like
//! `$MATCH`/`$PREMATCH`/`$POSTMATCH`/`$LAST_PAREN_MATCH`), and
//! [`NodeKind::NumberedReferenceReadNode`] (`$1`..`$9`) respectively.
//!
//! `derived_from_braceless_interpolation?`'s `%i[dstr regexp xstr].include?
//! (node.parent&.type)` becomes "the immediate parent is an
//! [`NodeKind::EmbeddedVariableNode`]": whitequark embeds the reference node
//! itself as a direct child of the string/regexp/xstr node for a `#$1`-style
//! shorthand interpolation, while Prism always interposes an
//! `EmbeddedVariableNode` wrapper (holding just the `#` operator and the
//! variable/reference node) regardless of the enclosing literal kind. Either
//! way `node`'s own span covers only the reference (`$1`, not `#$1`), so
//! replacing just `node` with a `{...}`-wrapped expression reproduces
//! upstream's `corrector.replace(node, preferred_expression)` byte-for-byte.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt, NodeKind};

/// Avoid Perl-style regex back references.
#[derive(Debug, Clone)]
pub struct PerlBackrefs;

impl Rule for PerlBackrefs {
    const META: RuleMeta = RuleMeta {
        name: "Style/PerlBackrefs",
        department: Department::Style,
        summary: "Avoid Perl-style regex back references.",
        explanation: "\
Looks for uses of Perl-style regexp match backreferences and their English
versions like `$1`, `$2`, `$&`, `$MATCH`, `$PREMATCH`, etc.

```ruby
# bad
puts $1

# good
puts Regexp.last_match(1)
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::BackReferenceReadNode,
            NodeKind::GlobalVariableReadNode,
            NodeKind::NumberedReferenceReadNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let preferred = match node.kind() {
            NodeKind::NumberedReferenceReadNode => {
                let number = node.as_numbered_reference_read_node().expect("kind matched").number();
                format!("Regexp.last_match({number})")
            }
            NodeKind::BackReferenceReadNode => {
                let name = node.as_back_reference_read_node().expect("kind matched").name();
                match preferred_expression_for(name.as_slice()) {
                    Some(expr) => expr.to_string(),
                    None => return, // `$~`/`$+`: no concise `Regexp.last_match` equivalent.
                }
            }
            NodeKind::GlobalVariableReadNode => {
                let name = node.as_global_variable_read_node().expect("kind matched").name();
                match preferred_expression_for(name.as_slice()) {
                    Some(expr) => expr.to_string(),
                    None => return, // e.g. `$LAST_PAREN_MATCH`: no concise equivalent.
                }
            }
            _ => return,
        };

        let preferred = format!("{}{preferred}", constant_prefix(ctx));
        let original = String::from_utf8_lossy(ctx.text(node.span())).into_owned();
        let message = format!("Prefer `{preferred}` over `{original}`.");

        let is_braceless_interpolation =
            ctx.parent().is_some_and(|p| p.kind == NodeKind::EmbeddedVariableNode);
        let replacement =
            if is_braceless_interpolation { format!("{{{preferred}}}") } else { preferred };

        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(node.span(), replacement.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, node.span(), message, fix);
    }
}

/// RuboCop's `preferred_expression_to`. `$+`/`$LAST_PAREN_MATCH` is
/// deliberately not converted: it refers to the last group that actually
/// matched, which has no concise `Regexp.last_match` equivalent
/// (`Regexp.last_match(-1)` is the last group in the *pattern*, which may be
/// `nil`). `$~`/`$LAST_MATCH_INFO` is not in `PREFERRED_VARS` either.
fn preferred_expression_for(name: &[u8]) -> Option<&'static str> {
    match name {
        b"$&" | b"$MATCH" => Some("Regexp.last_match(0)"),
        b"$`" | b"$PREMATCH" => Some("Regexp.last_match.pre_match"),
        b"$'" | b"$POSTMATCH" => Some("Regexp.last_match.post_match"),
        _ => None,
    }
}

/// RuboCop's `constant_prefix`: `"::"` inside a `class`/`module` body (so the
/// reference resolves to the top-level `Regexp` rather than a same-named
/// constant nested in the enclosing namespace), otherwise `""`.
fn constant_prefix(ctx: &Context<'_>) -> &'static str {
    let in_namespace = ctx
        .ancestors()
        .iter()
        .any(|a| matches!(a.kind, NodeKind::ClassNode | NodeKind::ModuleNode));
    if in_namespace {
        "::"
    } else {
        ""
    }
}
