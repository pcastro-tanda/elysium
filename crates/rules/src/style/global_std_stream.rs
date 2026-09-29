//! `Style/GlobalStdStream`, ported from RuboCop's
//! `lib/rubocop/cop/style/global_std_stream.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{ext, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Use `%<gvar_name>s` instead of `%<const_name>s`.";

/// The short (unqualified) name of a bare or top-level `STDIN`/`STDOUT`/
/// `STDERR` constant reference, if `node` is one and its name matches one
/// of the standard streams. Namespaced references (`Foo::STDOUT`) are
/// excluded via `ext::is_bare_or_toplevel_const`, matching upstream's
/// `namespaced?`.
fn std_stream_short_name<'a>(node: &Node<'a>) -> Option<&'a [u8]> {
    if !ext::is_bare_or_toplevel_const(node) {
        return None;
    }
    let name = match node.kind() {
        NodeKind::ConstantReadNode => node.as_constant_read_node()?.name().as_slice(),
        NodeKind::ConstantPathNode => node.as_constant_path_node()?.name()?.as_slice(),
        _ => return None,
    };
    matches!(name, b"STDIN" | b"STDOUT" | b"STDERR").then_some(name)
}

/// RuboCop's `gvar_name`: `"$" + const_name.downcase`.
fn gvar_name(const_name: &[u8]) -> String {
    format!("${}", String::from_utf8_lossy(const_name).to_lowercase())
}

/// Enforces the use of `$stdout/$stderr/$stdin` instead of
/// `STDOUT/STDERR/STDIN`.
#[derive(Debug, Clone, Default)]
pub struct GlobalStdStream {
    /// The span of a bare std-stream constant that is the value of a
    /// `gvasgn` assigning it to its own matching global variable (RuboCop's
    /// `const_to_gvar_assignment?`, e.g. `$stdin = STDIN`), set while
    /// visiting the enclosing `GlobalVariableWriteNode` and consumed when
    /// that value node is visited next.
    skip_span: Option<Span>,
}

impl Rule for GlobalStdStream {
    const META: RuleMeta = RuleMeta {
        name: "Style/GlobalStdStream",
        department: Department::Style,
        summary: "Enforces the use of `$stdout/$stderr/$stdin` instead of `STDOUT/STDERR/STDIN`.",
        explanation: "`STDOUT/STDERR/STDIN` are constants, and while you can actually reassign \
            (possibly to redirect some stream) constants in Ruby, you'll get an interpreter \
            warning if you do so. Additionally, `$stdout/$stderr/$stdin` can safely be accessed \
            in a Ractor because they are ractor-local, while `STDOUT/STDERR/STDIN` will raise \
            `Ractor::IsolationError`.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::ConstantReadNode,
            NodeKind::ConstantPathNode,
            NodeKind::GlobalVariableWriteNode,
        ],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self::default())
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if let NodeKind::GlobalVariableWriteNode = node.kind() {
            let write = node.as_global_variable_write_node().expect("kind matched");
            let value = write.value();
            if let Some(const_name) = std_stream_short_name(&value) {
                if write.name().as_slice() == gvar_name(const_name).as_bytes() {
                    self.skip_span = Some(value.span());
                }
            }
            return;
        }

        if self.skip_span == Some(node.span()) {
            self.skip_span = None;
            return;
        }

        let Some(const_name) = std_stream_short_name(node) else { return };
        let gvar = gvar_name(const_name);
        let message = MSG
            .replace("%<gvar_name>s", &gvar)
            .replace("%<const_name>s", &String::from_utf8_lossy(const_name));

        ctx.report_with_fix(
            &Self::META,
            node.span(),
            message,
            Fix {
                applicability: Applicability::Unsafe,
                edits: vec![Edit::replace(node.span(), gvar.into_bytes())],
            },
        );
    }
}
