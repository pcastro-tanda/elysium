//! `Style/FileEmpty`, ported from RuboCop's
//! `lib/rubocop/cop/style/file_empty.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext::const_name, node::CallNode, Node, NodeExt as _, NodeKind};

fn message(file_class: &str, arg: &str) -> String {
    format!("Use `{file_class}.empty?({arg})` instead.")
}

/// Whether `node` is a bare (non-safe-navigation) reference to `File` or
/// `FileTest`, matching upstream's `(const {nil? cbase} {:File :FileTest})`.
fn is_file_const(node: &Node<'_>) -> bool {
    matches!(const_name(node).as_deref(), Some("File" | "FileTest"))
}

/// A plain (non-safe-navigation) call's sole argument, if it has exactly
/// one and no block.
fn sole_argument<'pr>(call: &CallNode<'pr>) -> Option<Node<'pr>> {
    if call.is_safe_navigation() || call.block().is_some() {
        return None;
    }
    let args: Vec<Node<'pr>> = call.arguments()?.arguments().iter().collect();
    if args.len() == 1 {
        Some(args[0])
    } else {
        None
    }
}

/// Upstream's `(send (send CONST :!) ...)` unwrap: `!x` is a plain,
/// argument-less, non-safe-navigation call named `!`.
fn unwrap_bang<'pr>(node: &Node<'pr>) -> (Node<'pr>, bool) {
    if let Some(call) = node.as_call_node() {
        if !call.is_safe_navigation()
            && call.arguments().is_none()
            && call.name().as_slice() == b"!"
        {
            if let Some(receiver) = call.receiver() {
                return (receiver, true);
            }
        }
    }
    (*node, false)
}

fn is_int_zero(node: &Node<'_>) -> bool {
    node.as_integer_node().is_some_and(|n| TryInto::<i32>::try_into(n.value()) == Ok(0))
}

fn is_empty_string_literal(node: &Node<'_>) -> bool {
    node.as_string_node().is_some_and(|s| s.unescaped().is_empty())
}

/// RuboCop's `bang`: an explicit `!` to splice onto the replacement when the
/// original comparison's polarity would otherwise flip. `receiver` is the
/// matched node's own (not bang-unwrapped) receiver.
fn bang(name: &[u8], receiver: &Node<'_>) -> &'static str {
    let (_, receiver_is_bang) = unwrap_bang(receiver);
    if (name == b"==" && receiver_is_bang) || (matches!(name, b">=" | b"!=") && !receiver_is_bang) {
        "!"
    } else {
        ""
    }
}

/// The `(CONST, arg)` pair this `node` resolves to under one of upstream's
/// seven `offensive?` pattern alternatives, if any.
fn offending<'pr>(call: &CallNode<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
    if call.is_safe_navigation() {
        return None;
    }
    let receiver = call.receiver()?;
    match call.name().as_slice() {
        // `CONST.zero?(ARG)` or `CONST.size(ARG).zero?`.
        b"zero?" => {
            if let Some(size_call) = receiver.as_call_node() {
                if size_call.name().as_slice() != b"size" || call.arguments().is_some() {
                    return None;
                }
                let inner_receiver = size_call.receiver()?;
                if !is_file_const(&inner_receiver) {
                    return None;
                }
                return sole_argument(&size_call).map(|arg| (inner_receiver, arg));
            }
            if !is_file_const(&receiver) {
                return None;
            }
            sole_argument(call).map(|arg| (receiver, arg))
        }
        // `CONST.read(ARG).empty?` / `CONST.binread(ARG).empty?`.
        b"empty?" => {
            if call.arguments().is_some() {
                return None;
            }
            let read_call = receiver.as_call_node()?;
            if !matches!(read_call.name().as_slice(), b"read" | b"binread") {
                return None;
            }
            let inner_receiver = read_call.receiver()?;
            if !is_file_const(&inner_receiver) {
                return None;
            }
            sole_argument(&read_call).map(|arg| (inner_receiver, arg))
        }
        // `CONST.size(ARG) {== >=} 0`, optionally with the size call
        // negated first (`!CONST.size(ARG) {== >=} 0`); `CONST.(bin)read(ARG)
        // {== !=} ''`, likewise optionally negated first.
        name @ (b"==" | b"!=" | b">=") => {
            let rhs = sole_argument(call)?;
            let (base, _) = unwrap_bang(&receiver);
            let base_call = base.as_call_node()?;
            let inner_receiver = base_call.receiver()?;
            if !is_file_const(&inner_receiver) {
                return None;
            }
            match base_call.name().as_slice() {
                b"size" if matches!(name, b"==" | b">=") && is_int_zero(&rhs) => {
                    sole_argument(&base_call).map(|arg| (inner_receiver, arg))
                }
                b"read" | b"binread"
                    if matches!(name, b"==" | b"!=") && is_empty_string_literal(&rhs) =>
                {
                    sole_argument(&base_call).map(|arg| (inner_receiver, arg))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Prefer to use `File.empty?('path/to/file')` when checking if a file is empty.
#[derive(Debug, Clone)]
pub struct FileEmpty {
    target_ruby_version: f32,
}

impl Rule for FileEmpty {
    const META: RuleMeta = RuleMeta {
        name: "Style/FileEmpty",
        department: Department::Style,
        summary: "Prefer to use `File.empty?('path/to/file')` when checking if a file is empty.",
        explanation: "\
Prefer to use `File.empty?('path/to/file')` when checking if a file is empty.

@safety
This cop is unsafe, because `File.size`, `File.read`, and `File.binread`
raise `ENOENT` exception when there is no file corresponding to the path,
while `File.empty?` does not raise an exception.

```ruby
# bad
File.zero?('path/to/file')
File.size('path/to/file') == 0
File.size('path/to/file') >= 0
File.size('path/to/file').zero?
File.read('path/to/file').empty?
File.binread('path/to/file') == ''
FileTest.zero?('path/to/file')

# good
File.empty?('path/to/file')
FileTest.empty?('path/to/file')
```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if self.target_ruby_version < 2.4 {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        let Some((const_node, arg_node)) = offending(&call) else { return };

        let file_class = String::from_utf8_lossy(ctx.text(const_node.span())).into_owned();
        let arg = String::from_utf8_lossy(ctx.text(arg_node.span())).into_owned();
        let prefix = bang(call.name().as_slice(), &call.receiver().expect("receiver matched"));
        let replacement = format!("{prefix}{file_class}.empty?({arg})");

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message(&file_class, &arg),
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(node.span(), replacement.into_bytes())],
            },
        );
    }
}
