//! `Layout/LeadingCommentSpace`, ported from RuboCop's
//! `lib/rubocop/cop/layout/leading_comment_space.rb`.

use std::collections::HashMap;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::Node;

/// RuboCop's `MSG`.
const MSG: &str = "Missing space after `#`.";

/// Checks whether comments have a leading space after the `#` denoting the
/// start of the comment.
#[derive(Debug, Clone)]
pub struct LeadingCommentSpace {
    annotations: AllowedAnnotations,
    allow_gemfile_ruby: bool,
    allow_yard_separator: bool,
    file: FileContext,
}

/// The `AllowDoxygenCommentStyle`/`AllowRBSInlineAnnotation`/
/// `AllowSteepAnnotation` options: each exempts one specific non-standard
/// comment-start pattern.
#[derive(Debug, Clone, Copy, Default)]
struct AllowedAnnotations {
    doxygen: bool,
    rbs_inline: bool,
    steep: bool,
}

/// Facts about the linted file's own basename (RuboCop's
/// `rackup_config_file?`/`gemfile?`), computed once in `file_start`.
#[derive(Debug, Clone, Copy, Default)]
struct FileContext {
    /// The linted file's basename is exactly `config.ru`.
    is_rackup_config_file: bool,
    /// The linted file's basename is exactly `Gemfile`.
    is_gemfile: bool,
}

impl Rule for LeadingCommentSpace {
    const META: RuleMeta = RuleMeta {
        name: "Layout/LeadingCommentSpace",
        department: Department::Layout,
        summary: "Comments should start with a space.",
        explanation: "\
A `#` that starts a comment should be followed by a space, so the comment
reads as prose rather than code.

```ruby
# bad
#Some comment

# good
# Some comment
```

The leading space is not required for RDoc's `#++`/`#--` block markers, a
`#:nodoc:`-style directive (unless `AllowRBSInlineAnnotation` exempts it
first), a shebang line (or its continuation lines, or a rackup `#\\` options
line in `config.ru`), or a sprockets-style `#=` directive.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[],
        config: &[
            ConfigOption {
                name: "AllowDoxygenCommentStyle",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Allow a comment starting with `#*` (a Doxygen block delimiter).",
            },
            ConfigOption {
                name: "AllowGemfileRubyComment",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "In a file named `Gemfile`, allow a `#ruby`-prefixed comment (RVM's \
                      version pin syntax).",
            },
            ConfigOption {
                name: "AllowRBSInlineAnnotation",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Allow a comment starting with `#:`, `#|`, or `#[...]` (an RBS::Inline \
                      annotation).",
            },
            ConfigOption {
                name: "AllowSteepAnnotation",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Allow a comment starting with `#:` or `#$` (a Steep type annotation).",
            },
            ConfigOption {
                name: "AllowYARDCommentBlockSeparator",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Allow a comment that is exactly `#-` (a YARD comment block separator).",
            },
        ],
        blind_spots: "\
A comment's text is read from the file as-is; a `#`-run followed only by a
non-ASCII space character is not recognised as needing a space, matching
RuboCop's `\\s`-based regular expressions.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            annotations: AllowedAnnotations {
                doxygen: options.bool("AllowDoxygenCommentStyle"),
                rbs_inline: options.bool("AllowRBSInlineAnnotation"),
                steep: options.bool("AllowSteepAnnotation"),
            },
            allow_gemfile_ruby: options.bool("AllowGemfileRubyComment"),
            allow_yard_separator: options.bool("AllowYARDCommentBlockSeparator"),
            file: FileContext::default(),
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let basename = ctx.source().path().file_name().and_then(|name| name.to_str());
        self.file.is_rackup_config_file = basename == Some("config.ru");
        self.file.is_gemfile = basename == Some("Gemfile");
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let _ = (node, ctx);
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        // RuboCop's `shebang_continuation?` reads back whether the previous
        // line's comment was itself flagged, so comments are walked in
        // source order with a running `line -> (shebang?, offended?)` map.
        let mut seen: HashMap<u32, (bool, bool)> = HashMap::new();

        for comment in ctx.comments().to_vec() {
            let text = ctx.text(comment.span);
            let is_shebang = text.starts_with(b"#!");
            let mut offended = false;

            if matches_offense_pattern(text)
                && !(comment.line == 1
                    && (is_shebang
                        || (self.file.is_rackup_config_file && text.starts_with(b"#\\"))))
                && !shebang_continuation(comment.line, is_shebang, &seen)
                && !(self.annotations.doxygen && text.starts_with(b"#*"))
                && !(self.allow_gemfile_ruby && self.file.is_gemfile && text.starts_with(b"#ruby"))
                && !(self.annotations.rbs_inline && is_rbs_inline_annotation(text))
                && !(self.annotations.steep && (text.starts_with(b"#$") || text.starts_with(b"#:")))
                && !(self.allow_yard_separator && is_yard_block_separator(text))
            {
                offended = true;
                let insert_at = comment.span.start + 1;
                ctx.report_with_fix(
                    &Self::META,
                    comment.span,
                    MSG,
                    Fix {
                        applicability: Applicability::Safe,
                        edits: vec![Edit::insert(insert_at, b" " as &[u8])],
                    },
                );
            }

            seen.insert(comment.line, (is_shebang, offended));
        }
    }
}

/// RuboCop's `/\A(?!#\+\+|#--)(#+[^#\s=])/`: one or more `#` followed by a
/// character that is neither `#`, blank, nor `=`, unless the comment starts
/// with the `RDoc` block markers `#++`/`#--`.
fn matches_offense_pattern(text: &[u8]) -> bool {
    if text.starts_with(b"#++") || text.starts_with(b"#--") {
        return false;
    }
    let mut index = 0;
    while text.get(index) == Some(&b'#') {
        index += 1;
    }
    if index == 0 {
        return false;
    }
    match text.get(index) {
        Some(&byte) => !(byte.is_ascii_whitespace() || byte == b'='),
        None => false,
    }
}

/// RuboCop's `shebang_continuation?`: a shebang comment past the first line
/// is exempt only when the directly preceding line is itself a shebang
/// comment that this cop did not flag.
fn shebang_continuation(line: u32, is_shebang: bool, seen: &HashMap<u32, (bool, bool)>) -> bool {
    if !is_shebang {
        return false;
    }
    if line == 1 {
        return true;
    }
    matches!(seen.get(&(line - 1)), Some(&(true, false)))
}

/// RuboCop's `rbs_inline_annotation?`:
/// `comment.text.start_with?(/#:|#\[.+\]|#\|/)`.
fn is_rbs_inline_annotation(text: &[u8]) -> bool {
    text.starts_with(b"#:")
        || text.starts_with(b"#|")
        || (text.starts_with(b"#[") && text.len() > 3 && text[3..].contains(&b']'))
}

/// RuboCop's `yard_comment_block_separator?`: `comment.text.match?(/\A#-\s*\z/)`.
fn is_yard_block_separator(text: &[u8]) -> bool {
    text.starts_with(b"#-") && text[2..].iter().all(u8::is_ascii_whitespace)
}
