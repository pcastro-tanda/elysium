//! `Layout/InitialIndentation`, ported from RuboCop's
//! `lib/rubocop/cop/layout/initial_indentation.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::NodeExt as _;
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Indentation of first line in file detected.";

/// Checks for indentation of the first non-blank non-comment line in a file.
#[derive(Debug, Clone, Default)]
pub struct InitialIndentation;

impl Rule for InitialIndentation {
    const META: RuleMeta = RuleMeta {
        name: "Layout/InitialIndentation",
        department: Department::Layout,
        summary: "Checks for indentation of the first non-blank non-comment line in a file.",
        explanation: "\
Checks for indentation of the first non-blank non-comment line in a file.

```ruby
# bad
   class A
     def foo; end
   end

# good
class A
  def foo; end
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[],
        blind_spots: "\
Upstream measures the offense's length as the parser's first lexical token
(`processed_source.tokens.find { |t| !t.text.start_with?('#') }`). Without a
token stream, this port re-lexes just that one token from the source bytes
at the AST's first-statement offset (which Prism already anchors past any
leading whitespace, comments, and byte order mark). The mini-lexer covers
identifiers/keywords, numbers, strings, symbols, `@`/`@@`/`$` variables, and
the common operators; an exotic first token (e.g. a `%`-literal with unusual
delimiters) may get a slightly off highlight width, though the reported
offset, message, and autocorrect are unaffected.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let Some(program) = ctx.parsed().root().as_program_node() else { return };
        let Some(first) = program.statements().body().first() else { return };
        let token_start = first.span().start;

        let bytes = ctx.source().bytes();
        let space_start = leading_space_start(bytes, token_start);
        if space_start == token_start {
            return;
        }

        let width = first_token_width(&bytes[token_start as usize..]);
        let token_span = Span::new(token_start, token_start + width);
        let space_span = Span::new(space_start, token_start);

        ctx.report_with_fix(
            &Self::META,
            token_span,
            MSG,
            Fix { applicability: Applicability::Safe, edits: vec![Edit::delete(space_span)] },
        );
    }
}

/// RuboCop's `range_with_surrounding_space(token.pos, side: :left,
/// newlines: false)`: expands left over a `[ \t]` run only, never crossing a
/// newline (so a blank or comment-only preceding line, or a byte order mark,
/// stops the scan immediately).
fn leading_space_start(bytes: &[u8], token_start: u32) -> u32 {
    let mut i = token_start;
    while i > 0 && matches!(bytes[(i - 1) as usize], b' ' | b'\t') {
        i -= 1;
    }
    i
}

/// Whether `b` can continue a plain Ruby identifier: ASCII alphanumeric,
/// underscore, or any non-ASCII (multi-)byte, since Ruby identifiers may
/// contain Unicode letters.
fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

/// Width, in bytes, of the single lexical token starting at `text[0]`.
/// `text` is assumed to start exactly at a real token (never whitespace, a
/// comment, or a byte order mark -- the caller already anchors past those).
fn first_token_width(text: &[u8]) -> u32 {
    let Some(&first) = text.first() else { return 0 };
    match first {
        b'a'..=b'z' | b'A'..=b'Z' | b'_' | 0x80..=0xFF => {
            let mut i = 1;
            while i < text.len() && is_ident_byte(text[i]) {
                i += 1;
            }
            if i < text.len() && matches!(text[i], b'?' | b'!') {
                i += 1;
            }
            u32::try_from(i).expect("offset exceeds u32")
        }
        b'0'..=b'9' => number_width(text),
        b':' => symbol_width(text),
        b'"' | b'\'' | b'`' => string_width(text, first),
        b'@' => {
            let mut i = 1;
            if i < text.len() && text[i] == b'@' {
                i += 1;
            }
            while i < text.len() && is_ident_byte(text[i]) {
                i += 1;
            }
            u32::try_from(i).expect("offset exceeds u32")
        }
        b'$' => {
            let mut i = 1;
            if i < text.len() && text[i].is_ascii_digit() {
                while i < text.len() && text[i].is_ascii_digit() {
                    i += 1;
                }
            } else if i < text.len() && is_ident_byte(text[i]) {
                while i < text.len() && is_ident_byte(text[i]) {
                    i += 1;
                }
            } else if i < text.len() {
                i += 1;
            }
            u32::try_from(i).expect("offset exceeds u32")
        }
        b'%' => percent_width(text),
        _ => operator_width(text),
    }
}

/// Numeric literal width: decimal/hex/octal/binary digits, an optional
/// fractional part, an optional exponent, and an optional `r`/`i`
/// (rational/imaginary) suffix.
fn number_width(text: &[u8]) -> u32 {
    if text[0] == b'0'
        && text.len() > 1
        && matches!(text[1], b'x' | b'X' | b'o' | b'O' | b'b' | b'B' | b'd' | b'D')
    {
        let mut i = 2;
        while i < text.len() && (text[i].is_ascii_alphanumeric() || text[i] == b'_') {
            i += 1;
        }
        return u32::try_from(i).expect("offset exceeds u32");
    }

    let mut i = 0;
    while i < text.len() && (text[i].is_ascii_digit() || text[i] == b'_') {
        i += 1;
    }
    if i < text.len() && text[i] == b'.' && i + 1 < text.len() && text[i + 1].is_ascii_digit() {
        i += 1;
        while i < text.len() && (text[i].is_ascii_digit() || text[i] == b'_') {
            i += 1;
        }
    }
    if i < text.len() && matches!(text[i], b'e' | b'E') {
        let mut j = i + 1;
        if j < text.len() && matches!(text[j], b'+' | b'-') {
            j += 1;
        }
        if j < text.len() && text[j].is_ascii_digit() {
            i = j;
            while i < text.len() && text[i].is_ascii_digit() {
                i += 1;
            }
        }
    }
    if i < text.len() && matches!(text[i], b'r' | b'i') {
        i += 1;
    }
    u32::try_from(i).expect("offset exceeds u32")
}

/// Symbol literal width: `:` followed by a quoted string, a plain
/// identifier (with an optional trailing `?`/`!`/`=`), or an operator name
/// (e.g. `:+`, `:[]`).
fn symbol_width(text: &[u8]) -> u32 {
    if text.len() < 2 {
        return u32::try_from(text.len()).expect("offset exceeds u32");
    }
    match text[1] {
        b'"' | b'\'' => 1 + string_width(&text[1..], text[1]),
        b'a'..=b'z' | b'A'..=b'Z' | b'_' | 0x80..=0xFF => {
            let mut i = 1;
            while i < text.len() && is_ident_byte(text[i]) {
                i += 1;
            }
            if i < text.len() && matches!(text[i], b'?' | b'!' | b'=') {
                i += 1;
            }
            u32::try_from(i).expect("offset exceeds u32")
        }
        _ => 1 + operator_width(&text[1..]),
    }
}

/// Quoted-string width, `quote` being the opening (and closing) byte:
/// scans for the matching unescaped close quote. Single-quoted strings only
/// recognise `\\` and `\'` as escapes; every other quoting escapes any
/// following byte.
fn string_width(text: &[u8], quote: u8) -> u32 {
    let mut i = 1;
    while i < text.len() {
        if text[i] == b'\\' {
            let escapes = quote != b'\'' || matches!(text.get(i + 1), Some(b'\\' | b'\''));
            if escapes {
                i += 2;
                continue;
            }
        }
        if text[i] == quote {
            i += 1;
            break;
        }
        i += 1;
    }
    u32::try_from(i).expect("offset exceeds u32")
}

/// `%`-literal width (`%w[]`, `%i()`, `%q{}`, ...): an optional type letter,
/// then a delimited body, matching nested same-type bracket delimiters.
fn percent_width(text: &[u8]) -> u32 {
    let mut i = 1;
    if i < text.len() && text[i].is_ascii_alphabetic() {
        i += 1;
    }
    let Some(&open) = text.get(i) else { return u32::try_from(i).expect("offset exceeds u32") };
    let close = match open {
        b'(' => b')',
        b'[' => b']',
        b'{' => b'}',
        b'<' => b'>',
        c => c,
    };
    let nested = matches!(open, b'(' | b'[' | b'{' | b'<');
    i += 1;
    let mut depth: u32 = 1;
    while i < text.len() && depth > 0 {
        if text[i] == b'\\' {
            i += 2;
            continue;
        }
        if nested && text[i] == open {
            depth += 1;
        } else if text[i] == close {
            depth -= 1;
        }
        i += 1;
    }
    u32::try_from(i).expect("offset exceeds u32")
}

/// The known multi-character operators, longest first, so a prefix match
/// never stops short (e.g. `===` before `==`).
const OPERATORS: &[&[u8]] = &[
    b"<=>", b"===", b"**=", b"<<=", b">>=", b"&&=", b"||=", b"...", b"..", b"::", b"==", b"!=",
    b"<=", b">=", b"&&", b"||", b"<<", b">>", b"**", b"+=", b"-=", b"*=", b"/=", b"%=", b"|=",
    b"&=", b"^=", b"=~", b"!~", b"->", b"=>",
];

/// Width of a punctuation/operator token: the longest known multi-character
/// operator matching at the start of `text`, or a single byte.
fn operator_width(text: &[u8]) -> u32 {
    for op in OPERATORS {
        if text.len() >= op.len() && &text[..op.len()] == *op {
            return u32::try_from(op.len()).expect("offset exceeds u32");
        }
    }
    u32::from(!text.is_empty())
}
