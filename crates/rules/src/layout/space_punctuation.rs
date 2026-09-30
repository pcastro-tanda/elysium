//! Shared helpers for RuboCop's `SpaceAfterPunctuation` and
//! `SpaceBeforePunctuation` mixins
//! (`lib/rubocop/cop/mixin/space_after_punctuation.rb` /
//! `space_before_punctuation.rb`), used by `Layout/SpaceAfterComma`,
//! `Layout/SpaceAfterSemicolon`, `Layout/SpaceBeforeComma` and
//! `Layout/SpaceBeforeSemicolon`.
//!
//! RuboCop drives both mixins off `processed_source.tokens.each_cons(2)`
//! (`sorted_tokens` for the "before" mixin is the same sequence in file
//! order). Prism hands this engine no token stream, only node locations, so
//! these rules approximate one directly at the byte level instead: `,` and
//! `;` are always single ASCII bytes and never real punctuation when they
//! appear inside a string/regexp/symbol body or a comment
//! ([`Context::in_opaque_span`]), so a raw scan for those two bytes outside
//! every opaque span (via [`each_punctuation`]) reproduces exactly the
//! comma/semicolon tokens RuboCop's lexer would hand these mixins.
//!
//! Both mixins' `space_missing?` boils down to one adjacency test between
//! the punctuation and its neighboring real token -- same line, and not
//! separated by so much as a single space -- so neither needs the
//! neighbor's full token text, only whichever single byte immediately
//! touches the punctuation on the relevant side:
//!
//! - "after": the very next byte, unless the punctuation is already the
//!   last thing on its line ([`adjacent_after`]).
//! - "before": scanning backward from the punctuation to the start of its
//!   own line, stopping at (and returning) the first non-whitespace byte,
//!   if any ([`prev_on_line`]).
//!
//! `SpaceAfterPunctuation#allowed_type?`'s `tSTRING_DEND` -- the closing
//! `}` of a `#{...}` interpolation, which RuboCop's lexer tokenizes
//! distinctly from an ordinary `tRCURLY` and so always allows directly
//! adjacent to punctuation regardless of
//! `Layout/SpaceInside{Hash,Block}...Braces`'s configured style -- has no
//! raw-byte tell of its own (it is byte-for-byte the same `}` as any other
//! closing brace), so [`InterpolationClosers`] records every
//! `EmbeddedStatementsNode`'s closing delimiter position during the tree
//! walk for the "after" rules to consult.

use std::collections::HashSet;

use linter::{Context, OptionValue, RuleOptions};
use ruby_ast::{LocationExt as _, Node};
use ruby_source::is_ruby_whitespace;

/// Every offset in `ctx`'s source equal to `punct` that is real code -- not
/// inside a string/regexp/symbol body or a comment
/// ([`Context::in_opaque_span`]), and not past a `__END__` data section
/// (RuboCop's tokens never reach past one either).
pub(crate) fn each_punctuation(ctx: &Context<'_>, punct: u8, mut f: impl FnMut(u32)) {
    let bytes = ctx.source().bytes();
    let limit = ctx.parsed().data_span().map_or(bytes.len(), |span| span.start as usize);
    for (i, &b) in bytes[..limit].iter().enumerate() {
        if b == punct {
            let pos = u32::try_from(i).unwrap_or(u32::MAX);
            if !ctx.in_opaque_span(pos) {
                f(pos);
            }
        }
    }
}

/// The offset and value of the byte immediately after the single-byte
/// punctuation at `pos`, when it is not itself whitespace (a space/tab
/// keeps the punctuation correctly spaced; a newline or end-of-file both
/// mean nothing follows on the same line) -- RuboCop's `space_missing?`
/// collapsed to the one byte that can ever matter, since any gap at all
/// already fails that check.
pub(crate) fn adjacent_after(ctx: &Context<'_>, pos: u32) -> Option<(u32, u8)> {
    let bytes = ctx.source().bytes();
    let next = pos as usize + 1;
    let byte = *bytes.get(next)?;
    (!is_ruby_whitespace(byte)).then_some((u32::try_from(next).unwrap_or(u32::MAX), byte))
}

/// The first non-whitespace byte found scanning backward from `pos` to the
/// start of its own line, and the offset right after it -- RuboCop's
/// `space_missing?`'s `token1`, reduced to what `space_required_after?` (a
/// `left_curly_brace?` test) needs plus whether any gap separates it from
/// the punctuation at `pos` (no gap when the returned offset equals `pos`).
/// `None` when the punctuation begins its own line (no preceding token on
/// the same line, so never an offense).
pub(crate) fn prev_on_line(ctx: &Context<'_>, pos: u32) -> Option<(u32, u8)> {
    let line = ctx.line_col(pos).line;
    let line_start = ctx.line_span(line).start;
    let bytes = ctx.source().bytes();
    let mut i = pos;
    while i > line_start {
        let byte = bytes[i as usize - 1];
        if !is_ruby_whitespace(byte) {
            return Some((i, byte));
        }
        i -= 1;
    }
    None
}

/// `cfg.for_cop(cop)['EnforcedStyle'] || 'space'`, compared against `want`
/// (`'space'`/`'no_space'`) -- the shape both
/// `SpaceAfterPunctuation#space_style_before_rcurly` overrides
/// (`Layout/SpaceInsideHashLiteralBraces` for commas,
/// `Layout/SpaceInsideBlockBraces` for semicolons) and
/// `SpaceBeforePunctuation#space_required_after_lcurly?` (always
/// `Layout/SpaceInsideBlockBraces`) reduce to.
pub(crate) fn peer_brace_style_is(options: &RuleOptions, cop: &str, want: &str) -> bool {
    let style = match options.peer(cop, "EnforcedStyle") {
        Some(OptionValue::Str(s)) => s.as_str(),
        _ => "space",
    };
    style == want
}

/// Every `#{...}` closing-brace position in the file --
/// `EmbeddedStatementsNode::closing_loc`, recorded during the tree walk.
/// See the module docs for why the "after" rules need this.
#[derive(Debug, Clone, Default)]
pub(crate) struct InterpolationClosers(HashSet<u32>);

impl InterpolationClosers {
    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    pub(crate) fn record(&mut self, node: &Node<'_>) {
        if let Some(n) = node.as_embedded_statements_node() {
            self.0.insert(n.closing_loc().span().start);
        }
    }

    pub(crate) fn contains(&self, pos: u32) -> bool {
        self.0.contains(&pos)
    }
}
