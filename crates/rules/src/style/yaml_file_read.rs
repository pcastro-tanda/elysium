//! `Style/YAMLFileRead`, ported from RuboCop's
//! `lib/rubocop/cop/style/yaml_file_read.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::is_bare_or_toplevel_const;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks for the use of `YAML.load`, `YAML.safe_load`, and `YAML.parse` with `File.read`
/// argument.
#[derive(Debug, Clone)]
pub struct YAMLFileRead {
    target_ruby_version: f32,
}

/// The receiver of `node` is a bare or top-level-qualified constant named `name`
/// (RuboCop's `(const {cbase nil?} :NAME)`).
fn is_const_named(node: &Node<'_>, name: &[u8]) -> bool {
    let Some(receiver) = node.as_call_node().and_then(|c| c.receiver()) else { return false };
    if !is_bare_or_toplevel_const(&receiver) {
        return false;
    }
    match receiver.kind() {
        NodeKind::ConstantReadNode => {
            receiver.as_constant_read_node().is_some_and(|c| c.name().as_slice() == name)
        }
        NodeKind::ConstantPathNode => receiver
            .as_constant_path_node()
            .is_some_and(|p| p.name().is_some_and(|n| n.as_slice() == name)),
        _ => false,
    }
}

/// `YAML.load`/`YAML.safe_load`/`YAML.parse` called with a single
/// `File.read(path, ...)` argument: `(call (const {cbase nil?} :File) :read
/// $_) $...` -- returns the lone `File.read` argument (the file path) and
/// the rest of the outer call's own arguments.
fn yaml_file_read<'pr>(node: &Node<'pr>) -> Option<(Node<'pr>, Vec<Node<'pr>>)> {
    let call = node.as_call_node()?;
    let arguments = call.arguments()?;
    let args: Vec<Node<'pr>> = arguments.arguments().iter().collect();
    let (first, rest) = args.split_first()?;

    let file_read_call = first.as_call_node()?;
    if file_read_call.name().as_slice() != b"read" || !is_const_named(first, b"File") {
        return None;
    }
    let file_read_args: Vec<Node<'pr>> = file_read_call.arguments()?.arguments().iter().collect();
    let [file_path] = file_read_args.as_slice() else { return None };

    Some((*file_path, rest.to_vec()))
}

impl Rule for YAMLFileRead {
    const META: RuleMeta = RuleMeta {
        name: "Style/YAMLFileRead",
        department: Department::Style,
        summary: "Checks for the use of `YAML.load`, `YAML.safe_load`, and `YAML.parse` with \
                  `File.read` argument.",
        explanation: "",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { target_ruby_version: options.target_ruby_version() })
    }
    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        let method = call.name();
        let method = method.as_slice();
        if !matches!(method, b"load" | b"safe_load" | b"parse") {
            return;
        }
        if method == b"safe_load" && self.target_ruby_version <= 2.7 {
            return;
        }
        if !is_const_named(node, b"YAML") {
            return;
        }
        let Some((file_path, rest_arguments)) = yaml_file_read(node) else { return };

        let Some(message_loc) = call.message_loc() else { return };
        let span = Span::new(message_loc.span().start, node.span().end);

        let file_path_src = String::from_utf8_lossy(ctx.text(file_path.span())).into_owned();
        let rest_src = if rest_arguments.is_empty() {
            String::new()
        } else {
            let joined: Vec<String> = rest_arguments
                .iter()
                .map(|a| String::from_utf8_lossy(ctx.text(a.span())).into_owned())
                .collect();
            format!(", {}", joined.join(", "))
        };
        let method_str = String::from_utf8_lossy(method);
        let prefer = format!("{method_str}_file({file_path_src}{rest_src})");
        let msg = format!("Use `{prefer}` instead.");
        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, prefer.into_bytes())],
        };
        ctx.report_with_fix(&Self::META, span, msg, fix);
    }
}
