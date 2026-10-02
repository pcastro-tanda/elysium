//! `Style/FileTouch`, ported from RuboCop's
//! `lib/rubocop/cop/style/file_touch.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext::const_name, Node, NodeExt as _, NodeKind};

/// Upstream's `APPEND_FILE_MODES`.
const APPEND_FILE_MODES: &[&[u8]] = &[b"a", b"a+", b"ab", b"a+b", b"at", b"a+t"];

fn message(argument: &str) -> String {
    format!(
        "Use `FileUtils.touch({argument})` instead of `File.open` in append mode with empty block."
    )
}

/// Checks for usage of `File.open` in append mode with empty block.
#[derive(Debug, Clone)]
pub struct FileTouch;

impl Rule for FileTouch {
    const META: RuleMeta = RuleMeta {
        name: "Style/FileTouch",
        department: Department::Style,
        summary: "Favor `FileUtils.touch` for touching files.",
        explanation: "\
Checks for usage of `File.open` in append mode with empty block.

Such a usage only creates a new file, but it doesn't update
timestamps for an existing file, which might have been the intention.

For example, for an existing file `foo.txt`:

```
ruby -e \"puts File.mtime('foo.txt')\"
# 2024-11-26 12:17:23 +0100

ruby -e \"File.open('foo.txt', 'a') {}\"

ruby -e \"puts File.mtime('foo.txt')\"
# 2024-11-26 12:17:23 +0100 -> unchanged
```

If the intention was to update timestamps, `FileUtils.touch('foo.txt')`
should be used instead.

@safety
Autocorrection is unsafe for this cop because unlike `File.open`,
`FileUtils.touch` updates an existing file's timestamps.

```ruby
# bad
File.open(filename, 'a') {}
File.open(filename, 'a+') {}

# good
FileUtils.touch(filename)
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() || call.name().as_slice() != b"open" {
            return;
        }
        let Some(receiver) = call.receiver() else { return };
        if const_name(&receiver).as_deref() != Some("File") {
            return;
        }
        let Some(arguments) = call.arguments() else { return };
        let args: Vec<Node<'_>> = arguments.arguments().iter().collect();
        if args.len() != 2 {
            return;
        }
        let filename = &args[0];
        let mode = &args[1];
        let Some(mode_str) = mode.as_string_node() else { return };
        if !APPEND_FILE_MODES.contains(&mode_str.unescaped()) {
            return;
        }
        let Some(block) = call.block().as_ref().and_then(Node::as_block_node) else { return };
        if block.body().is_some() {
            return;
        }

        let argument = String::from_utf8_lossy(ctx.text(filename.span())).into_owned();
        let replacement = format!("FileUtils.touch({argument})");
        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message(&argument),
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(node.span(), replacement.into_bytes())],
            },
        );
    }
}
