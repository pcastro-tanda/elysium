//! `Style/RedundantLineContinuation`, ported from RuboCop's
//! `lib/rubocop/cop/style/redundant_line_continuation.rb` plus the
//! `ReparsedEquivalence`/`MatchRange` mixins it relies on.
//!
//! Upstream's `verified_by_reparse` applies each backslash-removal
//! correction, reparses, and compares the resulting `Parser::AST::Node` tree
//! to the original with `==` (type + children, location-blind). This port
//! has no generic value-equality over every one of Prism's ~150 node types,
//! but gets an equivalent (in fact stronger) check for free: removing one
//! backslash byte at offset `O` shifts every later byte left by exactly one,
//! so [`signature`]/[`shifted_signature`] walk the original and the
//! reparsed-from-modified-source tree collecting `(kind, start, end)` for
//! every node in preorder, shifting the original's offsets past `O` by `-1`
//! to match. Two preorder sequences of exact (kind, start, end) triples
//! being equal implies the trees are isomorphic down to which source bytes
//! belong to which node -- a strictly more precise test than value equality,
//! since it also pins down every leaf's exact text (any node whose content
//! differs would end up a different kind or claim different bytes).
//!
//! Performance: upstream batches candidates sharing a method/class/module
//! "reparse scope" into one reparse, falling back to one reparse per
//! candidate only when the batch fails. This port always reparses one
//! candidate at a time (one whole-file parse per `\`), which is simpler and,
//! for every fixture here, produces the same verdicts -- the only case where
//! this could diverge from upstream is a group of backslashes that only
//! parses equivalently when *all* are removed together but not individually
//! (noted in `blind_spots`; no fixture exercises it).
//!
//! `leading_dot_method_chain_with_blank_line?`'s guard against the `parser`
//! gem's own quirk (accepting a leading-dot chain continued across a blank
//! line, which breaks on MRI) is not ported: the cop's own comment notes
//! "Prism matches Ruby here and does not need this guard", so the plain
//! reparse check already gives the right answer for Prism.
//!
//! One grammar quirk runs the other way, though: unlike whitequark/CRuby
//! (`foo\n  && bar` is a `SyntaxError: unexpected tAMPER` on both), Prism
//! happily parses a bare `&&`/`||` starting the line right after a dropped
//! backslash as a continuation of the previous line's expression, producing
//! the exact same `AndNode`/`OrNode` either way. Reparse verification alone
//! can therefore never see these as non-equivalent, even though upstream
//! (and real Ruby) require the backslash. [`continues_into_leading_and_or_or`]
//! compensates by never treating a backslash as redundant when the next
//! line begins with `&&`/`||`, regardless of what reparsing says.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{walk, LocationExt as _, Node, NodeExt as _, NodeKind, ParseOptions, Parsed};
use ruby_source::{SourceFile, Span};

const MSG: &str = "Redundant line continuation.";

/// Prism-specific compensation (see the module doc): is the line right
/// after the backslash+newline at `offset` one that begins (after leading
/// spaces/tabs) with `&&` or `||`?
fn continues_into_leading_and_or_or(source: &[u8], offset: u32) -> bool {
    let mut i = offset as usize + 2;
    while i < source.len() && (source[i] == b' ' || source[i] == b'\t') {
        i += 1;
    }
    source[i..].starts_with(b"&&") || source[i..].starts_with(b"||")
}

/// One entry of a tree's preorder `(kind, start, end)` signature.
type SigEntry = (NodeKind, u32, u32);

/// Collects `root`'s preorder `(kind, start, end)` signature, applying
/// `shift` to every offset (identity for a reparsed tree; "subtract one past
/// the removed backslash" for the original tree -- see the module doc).
fn signature_with(root: &Node<'_>, shift: impl Fn(u32) -> u32) -> Vec<SigEntry> {
    struct Collector<F> {
        out: Vec<SigEntry>,
        shift: F,
    }
    impl<'pr, F: Fn(u32) -> u32> ruby_ast::Visitor<'pr> for Collector<F> {
        fn enter(&mut self, node: &Node<'pr>) {
            let span = node.span();
            self.out.push((node.kind(), (self.shift)(span.start), (self.shift)(span.end)));
        }
    }
    let mut collector = Collector { out: Vec::new(), shift };
    walk(root, &mut collector);
    collector.out
}

/// Whether removing the single byte at `backslash_offset` from `ctx`'s
/// source reparses to the same tree (RuboCop's `corrections_verify?` for a
/// single-item group): the modified source must parse cleanly, and its
/// preorder signature must equal the original's, shifted past the removed
/// byte.
fn verified_by_reparse(ctx: &Context<'_>, backslash_offset: u32) -> bool {
    let bytes = ctx.source().bytes();
    let cut = backslash_offset as usize;
    let mut modified = Vec::with_capacity(bytes.len() - 1);
    modified.extend_from_slice(&bytes[..cut]);
    modified.extend_from_slice(&bytes[cut + 1..]);

    let modified_file = SourceFile::new(ctx.source().path().to_path_buf(), modified);
    let reparsed = Parsed::parse_with(&modified_file, ParseOptions::default());
    if reparsed.has_errors() {
        return false;
    }

    let original_sig =
        signature_with(&ctx.parsed().root(), |x| if x > backslash_offset { x - 1 } else { x });
    let modified_sig = signature_with(&reparsed.root(), |x| x);
    original_sig == modified_sig
}

/// Checks for redundant line continuation.
///
/// A line continuation is redundant when removing the backslash does not
/// change how the program parses: the source is reparsed without the
/// backslash and the resulting AST is compared to the original. Only
/// backslashes that are pure noise are reported; backslashes that are
/// significant -- inside strings, for string concatenation, before an
/// operator or argument that would otherwise start a new statement, and so
/// on -- are left alone, as are backslashes in comments.
///
/// # Examples
///
/// ```ruby
/// # bad
/// foo. \
///   bar
///
/// # good
/// foo.
///   bar
///
/// # bad
/// foo(bar, \
///   baz)
///
/// # good
/// foo(bar,
///   baz)
///
/// # also good - backslash in string concatenation is not redundant
/// foo('bar' \
///   'baz')
/// ```
#[derive(Debug, Clone)]
pub struct RedundantLineContinuation;

impl Rule for RedundantLineContinuation {
    const META: RuleMeta = RuleMeta {
        name: "Style/RedundantLineContinuation",
        department: Department::Style,
        summary: "Checks for redundant line continuation.",
        explanation: "\
A line continuation is redundant when removing the backslash does not change \
how the program parses: the source is reparsed without the backslash and the \
resulting AST is compared to the original. Only backslashes that are pure \
noise are reported; backslashes that are significant -- inside strings, for \
string concatenation, before an operator or argument that would otherwise \
start a new statement, and so on -- are left alone, as are backslashes in \
comments.",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "\
Candidates sharing a method/class/module scope are verified one at a time \
instead of upstream's batch-then-fallback reparse grouping; a set of \
backslashes that only parses equivalently when removed *together* (but not \
individually) would be missed here. No fixture exercises this. The `parser`
gem's leading-dot/blank-line special case is intentionally not ported (see \
module doc): Prism's own grammar does not share that quirk.

fixtures/style/redundant_line_continuation/does_not_register_an_offense_when_a_line_continuation_prec_2.rb
is unfixable as generated: its upstream spec is an `expect_no_offenses` over
a four-line backslash-continued `1 & 2 | 3 ^ 4` expression, but the fixture
harness's own annotation parser treats the literal fourth line (`^ 4`,
Ruby's bitwise-xor operator applied to 4) as a caret offense-annotation line
and strips it, so this rule only ever sees the first three lines. Given
that truncated input, the final backslash genuinely is redundant (it
dangles at true end-of-file with nothing left to continue into, so removing
it reparses identically) -- the same verdict upstream's own
`verified_by_reparse` would reach on this mangled input. This is a
fixture-generation artifact, not a cop defect.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let program_span = ctx.parsed().root().location().span();
        let source = ctx.source().bytes();

        let mut candidates: Vec<u32> = Vec::new();
        let mut i = 0usize;
        while i + 1 < source.len() {
            if source[i] == b'\\' && source[i + 1] == b'\n' {
                let offset = u32::try_from(i).expect("offset exceeds u32");
                let in_program = offset >= program_span.start
                    && offset + 2 <= program_span.end
                    && !ctx.in_opaque_span(offset);
                if in_program {
                    candidates.push(offset);
                }
            }
            i += 1;
        }

        for offset in candidates {
            if continues_into_leading_and_or_or(source, offset) {
                continue;
            }
            if verified_by_reparse(ctx, offset) {
                // `LINE_CONTINUATION_PATTERN` matches the backslash and its newline.
                report(ctx, Span::new(offset, offset + 2));
            }
        }

        inspect_trailing_continuation(ctx, program_span);
    }
}

/// RuboCop's `inspect_end_of_ruby_code_line_continuation`: a backslash
/// ending the program's own last line, with nothing left for it to continue
/// into (so it sits outside `ast.source_range` and the main scan never
/// reaches it).
fn inspect_trailing_continuation(ctx: &mut Context<'_>, program_span: Span) {
    if program_span.end == program_span.start {
        return;
    }
    let last_line = ctx.line_col(program_span.end - 1).line;
    let line_text = ctx.line_text(last_line);
    if !line_text.ends_with(b"\\") {
        return;
    }
    let line_span = ctx.line_span(last_line);
    let offset = line_span.end - 1;
    if ctx.in_opaque_span(offset) {
        return;
    }
    if verified_by_reparse(ctx, offset) {
        // `trailing_line_continuation_range`: the backslash alone.
        report(ctx, Span::new(offset, offset + 1));
    }
}

/// The offense covers `range`; the correction removes only its leading
/// backslash (`remove_leading(range, 1)` / `remove_trailing(range, 1)`).
fn report(ctx: &mut Context<'_>, range: Span) {
    let backslash = Span::new(range.start, range.start + 1);
    let fix = Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(backslash)] };
    ctx.report_with_fix(&RedundantLineContinuation::META, range, MSG, fix);
}
