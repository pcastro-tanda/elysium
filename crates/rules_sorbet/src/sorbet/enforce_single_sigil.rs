//! `Sorbet/EnforceSingleSigil`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/sigils/enforce_single_sigil.rb`.

use std::sync::OnceLock;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use regex::bytes::Regex;
use ruby_source::Span;

const MSG: &str = "Files must only contain one sigil";

/// Checks that there is only one Sorbet sigil in a given file.
#[derive(Debug, Clone)]
pub struct EnforceSingleSigil;

impl Rule for EnforceSingleSigil {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/EnforceSingleSigil",
        department: Department::Sorbet,
        summary: "Ensures that there is only one Sorbet sigil in a file.",
        explanation: "Checks that there is only one Sorbet sigil in a given file.\n\nThe first \
                      sigil encountered represents the \"real\" strictness, so the following \
                      ones are removed by autocorrect. Other comments or magic comments are \
                      left in place.",
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
        let sigils = extract_all_sigils(ctx);
        if sigils.len() < 2 {
            return;
        }
        // The first sigil is the "real" strictness; remove any below.
        let edits: Vec<Edit> = sigils[1..]
            .iter()
            .map(|&token| {
                let line = ctx.line_col(token.start).line;
                let begin = ctx.line_span(line).start;
                Edit::delete(Span::new(begin, token.end + 1))
            })
            .collect();
        for &token in &sigils[1..] {
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

/// Ruby's lexer whitespace between tokens.
fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

/// The leading comment tokens (`take_while { tCOMMENT }`) matching
/// `SIGIL_REGEX`.
fn extract_all_sigils(ctx: &Context<'_>) -> Vec<Span> {
    let source = ctx.source().bytes();
    let mut comments = ctx.comments().iter().peekable();
    let mut pos = 0usize;
    let mut sigils = Vec::new();
    loop {
        while pos < source.len() && is_ws(source[pos]) {
            pos += 1;
        }
        let at = u32::try_from(pos).expect("offset exceeds u32");
        match comments.peek() {
            Some(comment) if comment.span.start == at => {
                if sigil_regex().is_match(ctx.text(comment.span)) {
                    sigils.push(comment.span);
                }
                pos = comment.span.end as usize;
                comments.next();
            }
            _ => break,
        }
    }
    sigils
}
