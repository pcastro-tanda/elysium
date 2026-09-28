//! `Lint/DuplicateRequire`, ported from RuboCop's
//! `lib/rubocop/cop/lint/duplicate_require.rb`.
//!
//! Upstream keys its per-scope `Set` of seen `require`s by `node.parent`
//! (`compare_by_identity`), resetting the set whenever the enclosing
//! statement list changes (e.g. crossing into a `def` body). Prism always
//! gives every node a real parent (no whitequark `begin`-elision to worry
//! about), and [`Context::parent`] returns that immediate parent's span,
//! which is just as good a per-scope identity key as object identity.
//! `node.first_argument.to_s` (a `Parser::AST::Node#to_s`, which renders by
//! *value*, e.g. `'foo'` and `"foo"` sexp identically) is stood in for by
//! the argument's unescaped string value when it is a plain string
//! literal, falling back to raw source text for anything else `_` could
//! still match (e.g. `__FILE__`, a `dstr`).

use std::collections::{HashMap, HashSet};

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks for duplicate `require`s and `require_relative`s.
#[derive(Debug, Clone, Default)]
pub struct DuplicateRequire {
    /// Seen `(method, argument)` keys per enclosing statement list, keyed by
    /// that list's span (RuboCop's `@required`, keyed by `node.parent`).
    required: HashMap<Span, HashSet<String>>,
}

impl Rule for DuplicateRequire {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DuplicateRequire",
        department: Department::Lint,
        summary: "Checks for duplicate `require`s and `require_relative`s.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let call = node.as_call_node().expect("kind matched");
        let Some(argument) = require_argument(&call) else { return };
        let method = call.name().as_slice();

        let mut key = String::from_utf8_lossy(method).into_owned();
        match argument.as_string_node() {
            Some(s) => key.push_str(&String::from_utf8_lossy(s.unescaped())),
            None => key.push_str(&String::from_utf8_lossy(ctx.text(argument.span()))),
        }

        let scope = ctx.parent().map_or(Span::empty(0), |p| p.span);
        if self.required.entry(scope).or_default().insert(key) {
            return;
        }

        let message = format!("Duplicate `{}` detected.", String::from_utf8_lossy(method));
        let deleted = ctx.whole_lines(node.span());
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix { applicability: Applicability::Unsafe, edits: vec![Edit::delete(deleted)] },
        );
    }
}

/// RuboCop's `require_call?`: `(send {nil? (const _ :Kernel)}
/// %REQUIRE_METHODS _)`. Returns the single argument when it matches.
fn require_argument<'pr>(call: &ruby_ast::node::CallNode<'pr>) -> Option<Node<'pr>> {
    let name = call.name().as_slice();
    if name != b"require" && name != b"require_relative" {
        return None;
    }
    if let Some(receiver) = call.receiver() {
        let name = ext::const_name(&receiver)?;
        if name != "Kernel" && !name.ends_with("::Kernel") {
            return None;
        }
    }
    let arguments = call.arguments()?;
    if arguments.arguments().len() != 1 {
        return None;
    }
    arguments.arguments().first()
}
