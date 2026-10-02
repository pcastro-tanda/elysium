//! `Rails/RootPathnameMethods`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/root_pathname_methods.rb`.

use std::collections::HashSet;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, OptionValue, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{call_span_excluding_block, const_name, is_bare_or_toplevel_const};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const DIR_GLOB_METHODS: &[&str] = &["[]", "glob"];

const DIR_NON_GLOB_METHODS: &[&str] = &[
    "children",
    "delete",
    "each_child",
    "empty?",
    "entries",
    "exist?",
    "mkdir",
    "open",
    "rmdir",
    "unlink",
];

const FILE_METHODS: &[&str] = &[
    "atime",
    "basename",
    "binread",
    "binwrite",
    "birthtime",
    "blockdev?",
    "chardev?",
    "chmod",
    "chown",
    "ctime",
    "delete",
    "directory?",
    "dirname",
    "empty?",
    "executable?",
    "executable_real?",
    "exist?",
    "expand_path",
    "extname",
    "file?",
    "fnmatch",
    "fnmatch?",
    "ftype",
    "grpowned?",
    "join",
    "lchmod",
    "lchown",
    "lstat",
    "mtime",
    "open",
    "owned?",
    "pipe?",
    "read",
    "readable?",
    "readable_real?",
    "readlines",
    "readlink",
    "realdirpath",
    "realpath",
    "rename",
    "setgid?",
    "setuid?",
    "size",
    "size?",
    "socket?",
    "split",
    "stat",
    "sticky?",
    "symlink?",
    "sysopen",
    "truncate",
    "unlink",
    "utime",
    "world_readable?",
    "world_writable?",
    "writable?",
    "writable_real?",
    "write",
    "zero?",
];

const FILE_TEST_METHODS: &[&str] = &[
    "blockdev?",
    "chardev?",
    "directory?",
    "empty?",
    "executable?",
    "executable_real?",
    "exist?",
    "file?",
    "grpowned?",
    "owned?",
    "pipe?",
    "readable?",
    "readable_real?",
    "setgid?",
    "setuid?",
    "size",
    "size?",
    "socket?",
    "sticky?",
    "symlink?",
    "world_readable?",
    "world_writable?",
    "writable?",
    "writable_real?",
    "zero?",
];

const FILE_UTILS_METHODS: &[&str] = &["chmod", "chown", "mkdir", "mkpath", "rmdir", "rmtree"];

/// Use `Rails.root` IO methods instead of passing it to `File`.
#[derive(Debug, Clone)]
pub struct RootPathnameMethods {
    /// `target_ruby_version >= 2.5`.
    glob_methods: bool,
    /// `Style/StringLiterals` `EnforcedStyle: double_quotes`.
    double_quotes: bool,
    /// Spans of the receivers and arguments of every non-safe-navigation
    /// call seen so far (the `parent&.send_type?` test).
    send_children: HashSet<(u32, u32)>,
}

impl Rule for RootPathnameMethods {
    const META: RuleMeta = RuleMeta {
        name: "Rails/RootPathnameMethods",
        department: Department::Rails,
        summary: "Use `Rails.root` IO methods instead of passing it to `File`.",
        explanation: "Use `Rails.root` IO methods instead of passing it to `File`.\n\n`Rails.root` \
                      is an instance of `Pathname` so we can apply many IO methods directly.\n\n\
                      This cop works best when used together with `Style/FileRead`, \
                      `Style/FileWrite` and `Rails/RootJoinChain`.\n\nThis cop is unsafe for \
                      autocorrection because ``Dir``'s `children`, `each_child`, `entries`, and \
                      `glob` methods return string element, but these methods of `Pathname` \
                      return `Pathname` element.\n\n```ruby\n# bad\nFile.open(Rails.root.join(\
                      'db', 'schema.rb'))\nFile.read(Rails.root.join('db', 'schema.rb'))\n\
                      Dir.glob(Rails.root.join('db', 'schema.rb'))\n\n# good\nRails.root.join(\
                      'db', 'schema.rb').open\nRails.root.join('db', 'schema.rb').read\n\
                      Rails.root.glob(\"db/schema.rb\")\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "`Dir.glob(Rails.root)` (no `join`), which makes RuboCop raise, reports \
                      nothing.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            glob_methods: options.target_ruby_version() >= 2.5,
            double_quotes: matches!(
                options.peer("Style/StringLiterals", "EnforcedStyle"),
                Some(OptionValue::Str(style)) if style == "double_quotes"
            ),
            send_children: HashSet::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let key = (node.span().start, node.span().end);
        let is_send_child = self.send_children.contains(&key);
        if !call.is_safe_navigation() && call.name().as_slice() != b"[]=" {
            if let Some(receiver) = call.receiver() {
                self.send_children.insert((receiver.span().start, receiver.span().end));
            }
            if let Some(arguments) = call.arguments() {
                for argument in arguments.arguments().iter() {
                    self.send_children.insert((argument.span().start, argument.span().end));
                }
            }
        }
        // `[]` is an `index` node, not a `send`.
        if call.name().as_slice() == b"[]" && call.receiver().is_some() {
            if let Some(arguments) = call.arguments() {
                for argument in arguments.arguments().iter() {
                    self.send_children.remove(&(argument.span().start, argument.span().end));
                }
            }
            if let Some(receiver) = call.receiver() {
                self.send_children.remove(&(receiver.span().start, receiver.span().end));
            }
        }
        if call.is_safe_navigation() {
            return;
        }
        let Some((method, path, args)) = self.pathname_method(&call) else { return };
        let has_block = call.block().is_some_and(|block| block.as_block_node().is_some());
        if method == "open" && is_send_child && !has_block {
            return;
        }
        let Some(rails_root) = rails_root_pathname(&path) else { return };

        let Some(replacement) = (if is_dir_glob(&call) {
            self.build_path_glob_replacement(&path, ctx)
        } else {
            Some(build_path_replacement(&path, method, &args, ctx))
        }) else {
            return;
        };
        let message = format!(
            "`{}` is a `Pathname`, so you can use `{replacement}`.",
            String::from_utf8_lossy(ctx.text(rails_root.span()))
        );
        let span = call_span_excluding_block(&call);
        ctx.report_with_fix(
            &Self::META,
            span,
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(span, replacement.into_bytes())],
            },
        );
    }
}

impl RootPathnameMethods {
    /// `pathname_method_for_ruby_2_5_or_higher` / `_2_4_or_lower`: the
    /// method name, the first argument and the remaining ones.
    fn pathname_method<'pr>(
        &self,
        call: &ruby_ast::node::CallNode<'pr>,
    ) -> Option<(&'static str, Node<'pr>, Vec<Node<'pr>>)> {
        let receiver = call.receiver()?;
        if !is_bare_or_toplevel_const(&receiver) {
            return None;
        }
        let name = std::str::from_utf8(call.name().as_slice()).ok()?;
        let candidates: &[&'static str] = match const_name(&receiver)?.as_str() {
            "Dir" => {
                if self.glob_methods && DIR_GLOB_METHODS.contains(&name) {
                    DIR_GLOB_METHODS
                } else {
                    DIR_NON_GLOB_METHODS
                }
            }
            "IO" | "File" => FILE_METHODS,
            "FileTest" => FILE_TEST_METHODS,
            "FileUtils" => FILE_UTILS_METHODS,
            _ => return None,
        };
        let method = candidates.iter().find(|candidate| **candidate == name)?;
        let mut arguments: Vec<Node<'pr>> =
            call.arguments().map(|list| list.arguments().iter().collect()).unwrap_or_default();
        if let Some(block) = call.block() {
            if block.as_block_argument_node().is_some() {
                arguments.push(block);
            }
        }
        if arguments.is_empty() {
            return None;
        }
        let path = arguments.remove(0);
        Some((method, path, arguments))
    }

    fn build_path_glob_replacement(&self, path: &Node<'_>, ctx: &Context<'_>) -> Option<String> {
        let path_call = path.as_call_node()?;
        let root = path_call.receiver()?.as_call_node()?.message_loc()?.span();
        let receiver = String::from_utf8_lossy(ctx.text(Span::new(path.span().start, root.end)));
        let arguments: Vec<Node<'_>> =
            path_call.arguments().map(|list| list.arguments().iter().collect()).unwrap_or_default();
        let argument = if let [only] = arguments.as_slice() {
            String::from_utf8_lossy(ctx.text(only.span())).into_owned()
        } else {
            self.join_arguments(&arguments, ctx)
        };
        Some(format!("{receiver}.glob({argument})"))
    }

    fn join_arguments(&self, arguments: &[Node<'_>], ctx: &Context<'_>) -> String {
        let mut use_interpolation = false;
        let joined: Vec<String> = arguments
            .iter()
            .map(|argument| {
                literal_value(argument, ctx).unwrap_or_else(|| {
                    use_interpolation = true;
                    format!("#{{{}}}", String::from_utf8_lossy(ctx.text(argument.span())))
                })
            })
            .collect();
        let quote = if self.double_quotes
            || arguments.iter().any(include_interpolation)
            || use_interpolation
        {
            '"'
        } else {
            '\''
        };
        format!("{quote}{}{quote}", joined.join("/"))
    }
}

/// `dir_glob?`.
fn is_dir_glob(call: &ruby_ast::node::CallNode<'_>) -> bool {
    call.receiver().is_some_and(|receiver| {
        is_bare_or_toplevel_const(&receiver) && const_name(&receiver).as_deref() == Some("Dir")
    }) && DIR_GLOB_METHODS.iter().any(|name| name.as_bytes() == call.name().as_slice())
}

/// `rails_root?`: `(send (const {nil? cbase} :Rails) {:root :public_path})`.
fn is_rails_root(node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node() else { return false };
    if call.is_safe_navigation()
        || call.arguments().is_some()
        || call.block().is_some()
        || !matches!(call.name().as_slice(), b"root" | b"public_path")
    {
        return false;
    }
    call.receiver().is_some_and(|receiver| {
        is_bare_or_toplevel_const(&receiver) && const_name(&receiver).as_deref() == Some("Rails")
    })
}

/// `rails_root_pathname?`: the captured `Rails.root` of `Rails.root` or
/// `Rails.root.join(...)`.
fn rails_root_pathname<'pr>(path: &Node<'pr>) -> Option<Node<'pr>> {
    if is_rails_root(path) {
        return Some(*path);
    }
    let call = path.as_call_node()?;
    if call.name().as_slice() != b"join" || call.is_safe_navigation() {
        return None;
    }
    if call.block().is_some_and(|block| block.as_block_node().is_some()) {
        return None;
    }
    let receiver = call.receiver()?;
    is_rails_root(&receiver).then_some(receiver)
}

fn build_path_replacement(
    path: &Node<'_>,
    method: &str,
    args: &[Node<'_>],
    ctx: &Context<'_>,
) -> String {
    let mut path_replacement = String::from_utf8_lossy(ctx.text(path.span())).into_owned();
    if let Some(call) = path.as_call_node() {
        if call.arguments().is_some() && call.opening_loc().is_none() {
            if let Some(index) = path_replacement.find(' ') {
                path_replacement.replace_range(index..=index, "(");
                path_replacement.push(')');
            }
        }
    }
    let mut replacement = format!("{path_replacement}.{method}");
    if !args.is_empty() {
        let formatted: Vec<String> = args
            .iter()
            .map(|arg| {
                let source = String::from_utf8_lossy(ctx.text(arg.span()));
                if arg.as_array_node().is_some() {
                    format!("*{source}")
                } else {
                    source.into_owned()
                }
            })
            .collect();
        replacement.push_str(&format!("({})", formatted.join(", ")));
    }
    replacement
}

/// `arg.value` for the nodes that respond to it (`str`, `dstr`, `sym`,
/// `int`, `float`, `rational`, `complex`).
fn literal_value(node: &Node<'_>, ctx: &Context<'_>) -> Option<String> {
    match node.kind() {
        NodeKind::StringNode => {
            Some(String::from_utf8_lossy(node.as_string_node()?.unescaped()).into_owned())
        }
        NodeKind::SymbolNode => {
            Some(String::from_utf8_lossy(node.as_symbol_node()?.unescaped()).into_owned())
        }
        NodeKind::InterpolatedStringNode => {
            let mut value = String::new();
            for part in node.as_interpolated_string_node()?.parts().iter() {
                match literal_value(&part, ctx) {
                    Some(text) => value.push_str(&text),
                    None => value.push_str(&String::from_utf8_lossy(ctx.text(part.span()))),
                }
            }
            Some(value)
        }
        NodeKind::IntegerNode
        | NodeKind::FloatNode
        | NodeKind::RationalNode
        | NodeKind::ImaginaryNode => Some(String::from_utf8_lossy(ctx.text(node.span())).into_owned()),
        _ => None,
    }
}

/// `include_interpolation?`: whether one of the argument's children is a
/// `begin` node (an interpolation or parenthesised expression).
fn include_interpolation(argument: &Node<'_>) -> bool {
    match argument.kind() {
        NodeKind::InterpolatedStringNode => argument.as_interpolated_string_node().is_some_and(|s| {
            s.parts().iter().any(|part| part.kind() == NodeKind::EmbeddedStatementsNode)
        }),
        NodeKind::CallNode => argument.as_call_node().is_some_and(|call| {
            call.receiver().is_some_and(|r| r.kind() == NodeKind::ParenthesesNode)
                || call.arguments().is_some_and(|list| {
                    list.arguments().iter().any(|a| a.kind() == NodeKind::ParenthesesNode)
                })
        }),
        _ => false,
    }
}
