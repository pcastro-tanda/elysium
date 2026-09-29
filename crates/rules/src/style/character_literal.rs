//! `Style/CharacterLiteral`, ported from RuboCop's
//! `lib/rubocop/cop/style/character_literal.rb` plus the `StringHelp` mixin
//! it includes (only the `offense?`/`autocorrect` hooks that mixin calls
//! into are relevant here).
//!
//! Prism parses a character literal (`?x`) as a plain `StringNode` whose
//! `opening_loc` is exactly `"?"` and whose `closing_loc` is absent --
//! unlike whitequark, which exposes a dedicated `character_literal?`
//! predicate. `?\C-\M-d` parses the same way (a `StringNode` with `"?"`
//! opening), so upstream's extra `node.source.size.between?(2, 3)` check
//! (on the *raw* source length, not the unescaped value) is what
//! distinguishes a plain character literal from a meta/control escape.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeKind};

const MSG: &str = "Do not use the character literal - use string literal instead.";

/// Checks for uses of character literals.
#[derive(Debug, Clone)]
pub struct CharacterLiteral;

impl Rule for CharacterLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Style/CharacterLiteral",
        department: Department::Style,
        summary: "Checks for uses of character literals.",
        explanation: "Checks for uses of the character literal ?x.\nStarting with Ruby 1.9 character literals are\nessentially one-character strings, so this syntax\nis mostly redundant at this point.\n\nA `?` character literal can be used to express meta and control characters.\nThat's a good use case of a `?` literal so it doesn't count as an offense.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StringNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(string) = node.as_string_node() else { return };
        let Some(opening) = string.opening_loc() else { return };
        if ctx.text(opening.span()) != b"?" {
            return;
        }
        let span = string.location().span();
        let source = ctx.text(span);
        // RuboCop's `node.source.size.between?(2, 3)`: the raw source
        // (including the leading `?`) is 2 or 3 bytes long, i.e. the part
        // after `?` is 1 or 2 bytes -- longer escapes like `\C-\M-d` are
        // meta/control characters and are left alone.
        if !(2..=3).contains(&source.len()) {
            return;
        }

        let rest = &source[1..];
        let replacement = if rest.len() == 2 || rest == b"'" {
            let mut text = Vec::with_capacity(rest.len() + 2);
            text.push(b'"');
            text.extend_from_slice(rest);
            text.push(b'"');
            Some(text)
        } else if rest.len() == 1 {
            let mut text = Vec::with_capacity(rest.len() + 2);
            text.push(b'\'');
            text.extend_from_slice(rest);
            text.push(b'\'');
            Some(text)
        } else {
            None
        };

        match replacement {
            Some(text) => {
                let fix = Fix {
                    applicability: Applicability::Safe,
                    edits: vec![Edit::replace(span, text)],
                };
                ctx.report_with_fix(&Self::META, span, MSG, fix);
            }
            None => ctx.report(&Self::META, span, MSG),
        }
    }
}
