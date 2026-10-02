//! `Style/FileNull`, ported from RuboCop's
//! `lib/rubocop/cop/style/file_null.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// Upstream's `valid_string?`: non-empty with no invalid byte sequences.
fn valid_string(value: &[u8]) -> bool {
    !value.is_empty() && std::str::from_utf8(value).is_ok()
}

/// Upstream's `REGEXP`: `/dev/null`, `NUL`, or `NUL:`, matched whole-string
/// and case-insensitively.
fn matches_null_device(lower: &str) -> bool {
    lower == "/dev/null" || lower == "nul" || lower == "nul:"
}

fn message(source: &str) -> String {
    format!("Use `File::NULL` instead of `{source}`.")
}

/// Use `File::NULL` instead of hardcoding the null device (`/dev/null` on Unix-like
/// OSes, `NUL` or `NUL:` on Windows), so that code is platform independent.
#[derive(Debug, Clone)]
pub struct FileNull {
    /// `@contain_dev_null_string_in_file`: whether any string literal in the
    /// file is exactly `/dev/null` (case-insensitively), recomputed per file
    /// since a bare `NUL` is only flagged when it is.
    contains_dev_null: bool,
}

impl Rule for FileNull {
    const META: RuleMeta = RuleMeta {
        name: "Style/FileNull",
        department: Department::Style,
        summary: "Use `File::NULL` instead of hardcoding \"dev/null\".",
        explanation: "\
Use `File::NULL` instead of hardcoding the null device (`/dev/null` on Unix-like
OSes, `NUL` or `NUL:` on Windows), so that code is platform independent.
Only looks for full string matches, substrings within a longer string are not
considered.

However, only files that use the string `'/dev/null'` are targeted for detection.
This is because the string `'NUL'` is not limited to the null device.
This behavior results in false negatives when the `'/dev/null'` string is not used,
but it is a trade-off to avoid false positives. `NULL:`
Unlike `'NUL'`, `'NUL:'` is regarded as something like `C:` and is always detected.

NOTE: Uses inside arrays and hashes are ignored.

@safety
It is possible for a string value to be changed if code is being run
on multiple platforms and was previously hardcoded to a specific null device.

For example, the following string will change on Windows when changed to
`File::NULL`:

```ruby
path = \"/dev/null\"
```

```ruby
# bad
'/dev/null'
'NUL'
'NUL:'

# good
File::NULL

# ok - inside an array
null_devices = %w[/dev/null nul]

# ok - inside a hash
{ unix: \"/dev/null\", windows: \"nul\" }
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::StringNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { contains_dev_null: false })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let mut found = false;
        ruby_ast::each_descendant(&root, &mut |node| {
            let Some(s) = node.as_string_node() else { return };
            let content = s.unescaped();
            if valid_string(content) && content.eq_ignore_ascii_case(b"/dev/null") {
                found = true;
            }
        });
        self.contains_dev_null = found;
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(string) = node.as_string_node() else { return };
        let content = string.unescaped();
        if !valid_string(content) {
            return;
        }
        if ctx.parent().is_some_and(|p| {
            matches!(
                p.kind,
                NodeKind::ArrayNode | NodeKind::AssocNode | NodeKind::InterpolatedStringNode
            )
        }) {
            return;
        }
        let lower = String::from_utf8_lossy(content).to_lowercase();
        if lower == "nul" && !self.contains_dev_null {
            return;
        }
        if !matches_null_device(&lower) {
            return;
        }
        let source = String::from_utf8_lossy(content).into_owned();
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message(&source),
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(node.span(), b"File::NULL".to_vec())],
            },
        );
    }
}
