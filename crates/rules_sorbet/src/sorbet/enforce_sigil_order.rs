//! `Sorbet/EnforceSigilOrder`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/sigils/enforce_sigil_order.rb`.

use std::sync::OnceLock;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::bytes::Regex;
use ruby_source::Span;

const MSG: &str = "Magic comments should be in the following order: encoding, typed, \
                   warn_indent, frozen_string_literal.";

/// Checks that the Sorbet sigil comes as the first magic comment in the file,
/// after the encoding comment if any.
#[derive(Debug, Clone)]
pub struct EnforceSigilOrder;

impl Rule for EnforceSigilOrder {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/EnforceSigilOrder",
        department: Department::Sorbet,
        summary: "Ensures that Sorbet sigil comes first in a file.",
        explanation: "Checks that the Sorbet sigil comes as the first magic comment in the file, \
                      after the encoding comment if any.\n\nThe expected order for magic \
                      comments is: (en)?coding, typed, warn_indent then \
                      frozen_string_literal.\n\nThe ordering is for consistency only, except \
                      for the encoding comment which must be first, if present.\n\nOnly \
                      `(en)?coding`, `typed`, `warn_indent` and `frozen_string_literal` magic \
                      comments are considered, other comments or magic comments are left in \
                      the same place.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let tokens = extract_magic_comments(ctx);
        if tokens.is_empty() {
            return;
        }

        // The current magic comments order.
        let mut order: Vec<usize> = Vec::new();
        for &token in &tokens {
            if let Some(i) = matches_of(ctx.text(token)).iter().position(|&m| m) {
                if !order.contains(&i) {
                    order.push(i);
                }
            }
        }
        // The expected order, based on the one used in the actual source.
        let expected_order: Vec<usize> =
            (0..4).filter(|&i| tokens.iter().any(|&t| matches_of(ctx.text(t))[i])).collect();

        if order == expected_order {
            return;
        }
        let edits = autocorrect(ctx, &tokens);
        for &token in &tokens {
            ctx.report_with_fix(
                &Self::META,
                token,
                MSG,
                Fix { applicability: Applicability::Safe, edits: edits.clone() },
            );
        }
    }
}

/// `SIGIL_REGEX` of `Sorbet::ValidSigil`.
fn sigil_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?m)^[[:blank:]]*(?:#[[:blank:]]*)?#[[:blank:]]+typed:(?:[[:blank:]]+((?-u:\S)+))?",
        )
        .expect("valid regex")
    })
}

/// `PREFERRED_ORDER` keys other than the sigil, in order: coding, indent,
/// frozen.
fn magic_regexes() -> &'static [Regex; 3] {
    static RE: OnceLock<[Regex; 3]> = OnceLock::new();
    RE.get_or_init(|| {
        [
            Regex::new(r"#\s+(en)?coding:(?:\s+([\w]+))?").expect("valid regex"),
            Regex::new(r"#\s+warn_indent:(?:\s+([\w]+))?").expect("valid regex"),
            Regex::new(r"#\s+frozen_string_literal:(?:\s+([\w]+))?").expect("valid regex"),
        ]
    })
}

/// Whether `text` matches each of `PREFERRED_ORDER.keys`: `coding`, `typed`,
/// `warn_indent`, `frozen_string_literal`.
fn matches_of(text: &[u8]) -> [bool; 4] {
    let [coding, indent, frozen] = magic_regexes();
    [
        coding.is_match(text),
        sigil_regex().is_match(text),
        indent.is_match(text),
        frozen.is_match(text),
    ]
}

/// Ruby's lexer whitespace between tokens.
fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

/// The leading comment tokens (`take_while { tCOMMENT }`) matching
/// `MAGIC_REGEX`.
fn extract_magic_comments(ctx: &Context<'_>) -> Vec<Span> {
    let source = ctx.source().bytes();
    let mut comments = ctx.comments().iter().peekable();
    let mut pos = 0usize;
    let mut tokens = Vec::new();
    loop {
        while pos < source.len() && is_ws(source[pos]) {
            pos += 1;
        }
        let at = u32::try_from(pos).expect("offset exceeds u32");
        match comments.peek() {
            Some(comment) if comment.span.start == at => {
                if matches_of(ctx.text(comment.span)).contains(&true) {
                    tokens.push(comment.span);
                }
                pos = comment.span.end as usize;
                comments.next();
            }
            _ => break,
        }
    }
    tokens
}

fn autocorrect(ctx: &Context<'_>, tokens: &[Span]) -> Vec<Edit> {
    // The magic comments tokens in their expected order.
    let expected: Vec<Span> = (0..4)
        .flat_map(|i| tokens.iter().copied().filter(move |&t| matches_of(ctx.text(t))[i]))
        .collect();

    let mut edits = Vec::new();
    for (index, &token) in tokens.iter().enumerate() {
        if let Some(&replacement) = expected.get(index) {
            edits.push(Edit::replace(token, ctx.text(replacement).to_vec()));
        }
    }

    // Remove blank lines between the magic comments.
    let lines: Vec<u32> = tokens.iter().map(|&t| ctx.line_col(t.start).line).collect();
    if let (Some(&min), Some(&max)) = (lines.iter().min(), lines.iter().max()) {
        for line in min..max {
            if lines.contains(&line) || !ctx.line_text(line).is_empty() {
                continue;
            }
            let start = ctx.line_span(line).start;
            edits.push(Edit::delete(Span::new(start, start + 1)));
        }
    }
    edits
}
