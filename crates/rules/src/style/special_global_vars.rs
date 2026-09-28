//! `Style/SpecialGlobalVars`, ported from RuboCop's
//! `lib/rubocop/cop/style/special_global_vars.rb` plus the `RequireLibrary`
//! mixin it includes for the `RequireEnglish` autocorrection.
//!
//! Only `on_gvar` (a global variable *read*) is ported; writes never reach
//! this cop upstream either. Numbered/back-references (`$1`, `$&`) are a
//! different Prism node (`NumberedReferenceReadNode`/`BackReferenceReadNode`),
//! so they never match `kinds` and need no special-casing.
//!
//! String interpolation (`#$foo`/`#{$foo}`) is a `EmbeddedVariableNode`/
//! `EmbeddedStatementsNode` parent in Prism, unlike whitequark's `begin`-node
//! climb; both cases replace the *parent's* full span (which already spans
//! the `#`/`#{`.."}" delimiters) with a self-contained `#name`/`#{name}`
//! replacement, matching upstream's `replacement`/`english_name_replacement`
//! output byte-for-byte.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)] // RuboCop style names
enum Style {
    EnglishNames,
    PerlNames,
    BuiltinEnglishNames,
}

/// Looks for uses of Perl-style global variables.
#[derive(Debug, Clone)]
pub struct SpecialGlobalVars {
    style: Style,
    require_english: bool,
    /// Whether a `require 'English'` has already been ensured for this file
    /// (either found pre-existing, or inserted by an earlier offense).
    required_english: bool,
}

impl Rule for SpecialGlobalVars {
    const META: RuleMeta = RuleMeta {
        name: "Style/SpecialGlobalVars",
        department: Department::Style,
        summary: "Avoid Perl-style global variables.",
        explanation: "\
Looks for uses of Perl-style global variables. Correcting to global
variables in the 'English' library will add a require statement to the top
of the file if enabled by the `RequireEnglish` config.

```ruby
# EnforcedStyle: use_english_names (default)

# good
require 'English' # or this could be in another file.

puts $LOAD_PATH
puts $LOADED_FEATURES
puts $PROGRAM_NAME
puts $ERROR_INFO
puts $ERROR_POSITION
puts $FIELD_SEPARATOR # or $FS
puts $OUTPUT_FIELD_SEPARATOR # or $OFS
puts $INPUT_RECORD_SEPARATOR # or $RS
puts $OUTPUT_RECORD_SEPARATOR # or $ORS
puts $INPUT_LINE_NUMBER # or $NR
puts $LAST_READ_LINE
puts $DEFAULT_OUTPUT
puts $DEFAULT_INPUT
puts $PROCESS_ID # or $PID
puts $CHILD_STATUS
puts $LAST_MATCH_INFO
puts $IGNORECASE
puts $ARGV # or ARGV
```

```ruby
# EnforcedStyle: use_perl_names

# good
puts $:
puts $\"
puts $0
puts $!
puts $@
puts $;
puts $,
puts $/
puts $\\
puts $.
puts $_
puts $>
puts $<
puts $$
puts $?
puts $~
puts $=
puts $*
```

```ruby
# EnforcedStyle: use_builtin_english_names

# good
# Like `use_perl_names` but allows builtin global vars.
puts $LOAD_PATH
puts $LOADED_FEATURES
puts $PROGRAM_NAME
puts ARGV
puts $:
puts $\"
puts $0
puts $!
puts $@
puts $;
puts $,
puts $/
puts $\\
puts $.
puts $_
puts $>
puts $<
puts $$
puts $?
puts $~
puts $=
puts $*
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::GlobalVariableReadNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("use_english_names"),
                allowed: &["use_perl_names", "use_english_names", "use_builtin_english_names"],
                doc: "Which set of global variable names to enforce.",
            },
            ConfigOption {
                name: "RequireEnglish",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether autocorrection to an 'English' library name should also insert \
                      a `require 'English'` at the top of the file.",
            },
        ],
        blind_spots: "\
`config_to_allow_offenses` auto-config generation (the `--auto-gen-config`
result when Perl/mixed styles are used) is not ported: this engine has no
config-generation pass.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "use_perl_names" => Style::PerlNames,
            "use_builtin_english_names" => Style::BuiltinEnglishNames,
            _ => Style::EnglishNames,
        };
        Ok(Self { style, require_english: options.bool("RequireEnglish"), required_english: false })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(gvar) = node.as_global_variable_read_node() else { return };
        let name = gvar.name();
        let global = name.as_slice();
        let Some(preferred) = preferred_names(self.style, global) else { return };
        if preferred.iter().any(|p| p.as_bytes() == global) {
            return; // Already using a preferred name for the configured style.
        }

        let global_str = String::from_utf8_lossy(global).into_owned();
        let message = self.message(&global_str, preferred);
        let fix = self.build_fix(ctx, node, preferred[0]);
        ctx.report_with_fix(&Self::META, node.span(), message, fix);
    }
}

impl SpecialGlobalVars {
    /// RuboCop's `message`/`format_english_message`/`format_message`.
    fn message(&self, global: &str, preferred: &'static [&'static str]) -> String {
        if self.style != Style::EnglishNames {
            return format!("Prefer `{}` over `{global}`.", preferred[0]);
        }
        let (regular, english): (Vec<&str>, Vec<&str>) =
            preferred.iter().copied().partition(|p| is_non_english_var(p));
        if regular.is_empty() {
            format!(
                "Prefer `{}` from the stdlib 'English' module (don't forget to require it) over \
                 `{global}`.",
                format_list(&english)
            )
        } else if english.is_empty() {
            format!("Prefer `{}` over `{global}`.", format_list(&regular))
        } else {
            format!(
                "Prefer `{}` from the stdlib 'English' module (don't forget to require it) or \
                 `{}` over `{global}`.",
                format_list(&english),
                format_list(&regular)
            )
        }
    }

    /// RuboCop's `autocorrect`/`replacement`/`english_name_replacement`, plus
    /// `should_require_english?`/`ensure_required` for the `RequireEnglish`
    /// insertion.
    fn build_fix(&mut self, ctx: &mut Context<'_>, node: &Node<'_>, preferred_name: &str) -> Fix {
        let mut edits = Vec::new();

        let ancestors = ctx.ancestors();
        let embedded_span = ancestors
            .iter()
            .rev()
            .take(2)
            .find(|a| {
                matches!(a.kind, NodeKind::EmbeddedVariableNode | NodeKind::EmbeddedStatementsNode)
            })
            .map(|a| a.span);
        let (target_span, replacement) = match embedded_span {
            Some(span) => {
                let text = if self.style == Style::EnglishNames {
                    format!("#{{{preferred_name}}}")
                } else {
                    format!("#{preferred_name}")
                };
                (span, text)
            }
            None => (node.span(), preferred_name.to_string()),
        };
        edits.push(Edit::replace(target_span, replacement.into_bytes()));

        if self.should_require_english(preferred_name) {
            ensure_required_english(ctx, node.span(), &mut edits);
            self.required_english = true;
        }

        Fix { applicability: Applicability::Unsafe, edits }
    }

    /// RuboCop's `should_require_english?`.
    fn should_require_english(&self, preferred_name: &str) -> bool {
        self.style == Style::EnglishNames
            && self.require_english
            && !self.required_english
            && !is_non_english_var(preferred_name)
    }
}

/// RuboCop's `NON_ENGLISH_VARS`.
fn is_non_english_var(name: &str) -> bool {
    matches!(name, "$LOAD_PATH" | "$LOADED_FEATURES" | "$PROGRAM_NAME" | "ARGV")
}

fn format_list(items: &[&str]) -> String {
    items.join("` or `")
}

/// RuboCop's `RequireLibrary#ensure_required`/`RequireLibraryCorrector`
/// specialised to the `'English'` library: finds the top-level statement
/// enclosing `offense_span`, and either does nothing (a `require 'English'`
/// already precedes it), or inserts `require 'English'` before the file's
/// first top-level statement and deletes any `require 'English'` that
/// appears *after* the enclosing statement (matching the removal of
/// now-redundant subsequent requires upstream performs).
fn ensure_required_english(ctx: &Context<'_>, offense_span: Span, edits: &mut Vec<Edit>) {
    let Some(program) = ctx.parsed().root().as_program_node() else { return };
    let stmts: Vec<(Span, Option<Vec<u8>>)> = program
        .statements()
        .body()
        .iter()
        .map(|stmt| (stmt.span(), require_target(&stmt)))
        .collect();
    let Some(current) = stmts
        .iter()
        .position(|(span, _)| span.start <= offense_span.start && offense_span.end <= span.end)
    else {
        return;
    };
    if stmts[..current].iter().any(|(_, req)| req.as_deref() == Some(b"English".as_slice())) {
        return; // Already required earlier in the file.
    }

    let Some((first_span, _)) = stmts.first() else { return };
    edits.push(Edit::insert(first_span.start, b"require 'English'\n".to_vec()));

    for (span, req) in &stmts[current + 1..] {
        if req.as_deref() == Some(b"English".as_slice()) {
            edits.push(Edit::delete(whole_line_with_newline(ctx, *span)));
        }
    }
}

/// RuboCop's `require_any_library?`/`require_library_name?`: a receiver-less
/// (or explicit top-level `Kernel`) `require` call with a single plain
/// string argument.
fn require_target(node: &Node<'_>) -> Option<Vec<u8>> {
    let call = node.as_call_node()?;
    if call.name().as_slice() != b"require" {
        return None;
    }
    if let Some(receiver) = call.receiver() {
        let is_kernel =
            receiver.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"Kernel")
                || receiver.as_constant_path_node().is_some_and(|p| {
                    p.parent().is_none() && p.name().is_some_and(|n| n.as_slice() == b"Kernel")
                });
        if !is_kernel {
            return None;
        }
    }
    let args = call.arguments()?.arguments();
    if args.len() != 1 {
        return None;
    }
    let string = args.first()?.as_string_node()?;
    Some(string.unescaped().to_vec())
}

/// RuboCop's `range_by_whole_lines(sibling.source_range, include_final_newline: true)`.
fn whole_line_with_newline(ctx: &Context<'_>, span: Span) -> Span {
    let line = ctx.line_col(span.start).line;
    let line_span = ctx.line_span(line);
    let bytes = ctx.source().bytes();
    let end = if bytes.get(line_span.end as usize) == Some(&b'\n') {
        line_span.end + 1
    } else {
        line_span.end
    };
    Span::new(line_span.start, end)
}

/// RuboCop's `STYLE_VARS_MAP`, resolved for the current style.
fn preferred_names(style: Style, global: &[u8]) -> Option<&'static [&'static str]> {
    match style {
        Style::EnglishNames => english_names(global),
        Style::PerlNames => perl_names(global),
        Style::BuiltinEnglishNames => builtin_english_names(global),
    }
}

/// RuboCop's `ENGLISH_VARS` (after its self-merge).
fn english_names(global: &[u8]) -> Option<&'static [&'static str]> {
    Some(match global {
        b"$:" | b"$LOAD_PATH" => &["$LOAD_PATH"],
        b"$\"" | b"$LOADED_FEATURES" => &["$LOADED_FEATURES"],
        b"$0" | b"$PROGRAM_NAME" => &["$PROGRAM_NAME"],
        b"$!" | b"$ERROR_INFO" => &["$ERROR_INFO"],
        b"$@" | b"$ERROR_POSITION" => &["$ERROR_POSITION"],
        b"$;" => &["$FIELD_SEPARATOR", "$FS"],
        b"$FIELD_SEPARATOR" => &["$FIELD_SEPARATOR"],
        b"$FS" => &["$FS"],
        b"$," => &["$OUTPUT_FIELD_SEPARATOR", "$OFS"],
        b"$OUTPUT_FIELD_SEPARATOR" => &["$OUTPUT_FIELD_SEPARATOR"],
        b"$OFS" => &["$OFS"],
        b"$/" => &["$INPUT_RECORD_SEPARATOR", "$RS"],
        b"$INPUT_RECORD_SEPARATOR" => &["$INPUT_RECORD_SEPARATOR"],
        b"$RS" => &["$RS"],
        b"$\\" => &["$OUTPUT_RECORD_SEPARATOR", "$ORS"],
        b"$OUTPUT_RECORD_SEPARATOR" => &["$OUTPUT_RECORD_SEPARATOR"],
        b"$ORS" => &["$ORS"],
        b"$." => &["$INPUT_LINE_NUMBER", "$NR"],
        b"$INPUT_LINE_NUMBER" => &["$INPUT_LINE_NUMBER"],
        b"$NR" => &["$NR"],
        b"$_" | b"$LAST_READ_LINE" => &["$LAST_READ_LINE"],
        b"$>" | b"$DEFAULT_OUTPUT" => &["$DEFAULT_OUTPUT"],
        b"$<" | b"$DEFAULT_INPUT" => &["$DEFAULT_INPUT"],
        b"$$" => &["$PROCESS_ID", "$PID"],
        b"$PROCESS_ID" => &["$PROCESS_ID"],
        b"$PID" => &["$PID"],
        b"$?" | b"$CHILD_STATUS" => &["$CHILD_STATUS"],
        b"$~" | b"$LAST_MATCH_INFO" => &["$LAST_MATCH_INFO"],
        b"$=" | b"$IGNORECASE" => &["$IGNORECASE"],
        b"$*" => &["$ARGV", "ARGV"],
        b"$ARGV" => &["$ARGV"],
        _ => return None,
    })
}

/// RuboCop's `PERL_VARS` (after its self-merge).
fn perl_names(global: &[u8]) -> Option<&'static [&'static str]> {
    Some(match global {
        b"$LOAD_PATH" | b"$:" => &["$:"],
        b"$LOADED_FEATURES" | b"$\"" => &["$\""],
        b"$PROGRAM_NAME" | b"$0" => &["$0"],
        b"$ERROR_INFO" | b"$!" => &["$!"],
        b"$ERROR_POSITION" | b"$@" => &["$@"],
        b"$FIELD_SEPARATOR" | b"$FS" | b"$;" => &["$;"],
        b"$OUTPUT_FIELD_SEPARATOR" | b"$OFS" | b"$," => &["$,"],
        b"$INPUT_RECORD_SEPARATOR" | b"$RS" | b"$/" => &["$/"],
        b"$OUTPUT_RECORD_SEPARATOR" | b"$ORS" | b"$\\" => &["$\\"],
        b"$INPUT_LINE_NUMBER" | b"$NR" | b"$." => &["$."],
        b"$LAST_READ_LINE" | b"$_" => &["$_"],
        b"$DEFAULT_OUTPUT" | b"$>" => &["$>"],
        b"$DEFAULT_INPUT" | b"$<" => &["$<"],
        b"$PROCESS_ID" | b"$PID" | b"$$" => &["$$"],
        b"$CHILD_STATUS" | b"$?" => &["$?"],
        b"$LAST_MATCH_INFO" | b"$~" => &["$~"],
        b"$IGNORECASE" | b"$=" => &["$="],
        b"$ARGV" | b"$*" => &["$*"],
        _ => return None,
    })
}

/// RuboCop's `BUILTIN_VARS`: `PERL_VARS` with `$LOAD_PATH`/`$LOADED_FEATURES`/
/// `$PROGRAM_NAME` (and their Perl spellings) additionally self-mapped.
fn builtin_english_names(global: &[u8]) -> Option<&'static [&'static str]> {
    Some(match global {
        b"$LOAD_PATH" | b"$:" => &["$LOAD_PATH"],
        b"$LOADED_FEATURES" | b"$\"" => &["$LOADED_FEATURES"],
        b"$PROGRAM_NAME" | b"$0" => &["$PROGRAM_NAME"],
        _ => return perl_names(global),
    })
}
