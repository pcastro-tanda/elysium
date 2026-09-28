//! `Lint/DeprecatedClassMethods`, ported from RuboCop's
//! `lib/rubocop/cop/lint/deprecated_class_methods.rb`.
//!
//! Upstream's `deprecated_class_method?` node matcher is a flat `{...}`
//! alternation over five whitequark `send` shapes; here it becomes a match
//! on the call's method name (mirroring `RESTRICT_ON_SEND`) plus, per arm,
//! the receiver/argument checks the corresponding pattern encoded. The
//! matcher's `(const {cbase nil?} :ENV)` shape -- a bare constant, optionally
//! prefixed with a leading `::`, but *not* nested under another constant --
//! is [`top_level_const_name`]: `ConstantReadNode` (no cbase) or a
//! `ConstantPathNode` with no `parent()` (a bare `::Name`); `Foo::ENV` is
//! neither and so never matches, just as upstream's pattern requires.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `PREFERRED_METHODS`.
fn preferred_methods(name: &[u8]) -> Option<&'static str> {
    match name {
        b"clone" | b"dup" => Some("to_h"),
        b"exists?" => Some("exist?"),
        b"gethostbyaddr" => Some("Addrinfo#getnameinfo"),
        b"gethostbyname" => Some("Addrinfo.getaddrinfo"),
        b"iterator?" => Some("block_given?"),
        _ => None,
    }
}

/// RuboCop's `Node#short_name` for a bare (unqualified, or `::`-qualified)
/// constant reference; `None` for anything else, including a constant
/// nested under another constant (`Foo::ENV`), which upstream's `(const
/// {cbase nil?} ...)` shape also excludes.
fn top_level_const_name<'pr>(node: &Node<'pr>) -> Option<&'pr [u8]> {
    match node.kind() {
        NodeKind::ConstantReadNode => Some(node.as_constant_read_node()?.name().as_slice()),
        NodeKind::ConstantPathNode => {
            let path = node.as_constant_path_node()?;
            if path.parent().is_some() {
                return None;
            }
            Some(path.name()?.as_slice())
        }
        _ => None,
    }
}

/// RuboCop's `socket_const?`.
fn is_socket_const(receiver: Option<&Node<'_>>) -> bool {
    receiver.and_then(top_level_const_name) == Some(b"Socket")
}

/// RuboCop's `dir_env_file_const?`.
fn is_dir_env_file_const(receiver: Option<&Node<'_>>) -> bool {
    matches!(receiver.and_then(top_level_const_name), Some(b"Dir" | b"ENV" | b"File"))
}

fn arg_count(call: &CallNode<'_>) -> usize {
    call.arguments().map_or(0, |a| a.arguments().len())
}

fn is_bool_literal(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::TrueNode | NodeKind::FalseNode)
}

/// RuboCop's `deprecated_class_method?`, narrowed per method name the way
/// `RESTRICT_ON_SEND` already narrows `on_send` calls to these eight names.
fn matches_deprecated_call(call: &CallNode<'_>, name: &[u8], receiver: Option<&Node<'_>>) -> bool {
    match name {
        b"clone" | b"dup" | b"freeze" => {
            receiver.and_then(top_level_const_name) == Some(b"ENV") && arg_count(call) == 0
        }
        b"exists?" => {
            matches!(receiver.and_then(top_level_const_name), Some(b"File" | b"Dir"))
                && arg_count(call) == 1
        }
        b"gethostbyaddr" | b"gethostbyname" => is_socket_const(receiver),
        b"attr" => {
            receiver.is_none()
                && arg_count(call) == 2
                && call.arguments().is_some_and(|args| {
                    args.arguments().iter().nth(1).is_some_and(|second| is_bool_literal(&second))
                })
        }
        b"iterator?" => receiver.is_none() && arg_count(call) == 0,
        _ => false,
    }
}

/// RuboCop's `MSG`.
const fn msg() -> &'static str {
    "`%<current>s` is deprecated in favor of `%<prefer>s`."
}

/// Checks for uses of the deprecated class method usages.
#[derive(Debug, Clone)]
pub struct DeprecatedClassMethods;

impl Rule for DeprecatedClassMethods {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DeprecatedClassMethods",
        department: Department::Lint,
        summary: "Checks for deprecated class method calls.",
        explanation: "\
Checks for uses of the deprecated class method usages.

```ruby
# bad
File.exists?(some_path)
Dir.exists?(some_path)
iterator?
attr :name, true
attr :name, false
ENV.freeze # Calling `Env.freeze` raises `TypeError` since Ruby 2.7.
ENV.clone
ENV.dup # Calling `Env.dup` raises `TypeError` since Ruby 3.1.
Socket.gethostbyname(host)
Socket.gethostbyaddr(host)

# good
File.exist?(some_path)
Dir.exist?(some_path)
block_given?
attr_accessor :name
attr_reader :name
ENV # `ENV.freeze` cannot prohibit changes to environment variables.
ENV.to_h
ENV.to_h # `ENV.dup` cannot dup `ENV`, use `ENV.to_h` to get a copy of `ENV` as a hash.
Addrinfo.getaddrinfo(nodename, service)
Addrinfo.tcp(host, port).getnameinfo
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
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
        if call.is_safe_navigation() {
            return;
        }
        let name = call.name().as_slice();
        if !matches!(
            name,
            b"attr"
                | b"clone"
                | b"dup"
                | b"exists?"
                | b"freeze"
                | b"gethostbyaddr"
                | b"gethostbyname"
                | b"iterator?"
        ) {
            return;
        }
        let receiver = call.receiver();
        if !matches_deprecated_call(&call, name, receiver.as_ref()) {
            return;
        }

        let offense_span =
            if is_socket_const(receiver.as_ref()) || is_dir_env_file_const(receiver.as_ref()) {
                let selector = call.message_loc().expect("send always has a selector").span();
                Span::new(node.span().start, selector.end)
            } else if name == b"attr" {
                node.span()
            } else {
                call.message_loc().expect("send always has a selector").span()
            };

        let current = String::from_utf8_lossy(ctx.text(offense_span)).into_owned();
        let prefer = if name == b"attr" {
            let args = call.arguments().expect("checked arg_count == 2");
            let first = args.arguments().first().expect("checked arg_count == 2");
            let second = args.arguments().iter().nth(1).expect("checked arg_count == 2");
            let preferred_attr_method =
                if ctx.text(second.span()) == b"true" { "attr_accessor" } else { "attr_reader" };
            let first_source = String::from_utf8_lossy(ctx.text(first.span())).into_owned();
            format!("{preferred_attr_method} {first_source}")
        } else if is_dir_env_file_const(receiver.as_ref()) {
            match preferred_methods(name) {
                Some(prefer) => {
                    let receiver_source = String::from_utf8_lossy(
                        ctx.text(receiver.as_ref().expect("checked").span()),
                    )
                    .into_owned();
                    format!("{receiver_source}.{prefer}")
                }
                None => "ENV".to_string(),
            }
        } else {
            preferred_methods(name).unwrap_or_default().to_string()
        };

        let message = msg().replacen("%<current>s", &current, 1).replacen("%<prefer>s", &prefer, 1);

        if is_socket_const(receiver.as_ref()) {
            // Upstream's autocorrect block early-returns for a `Socket`
            // receiver (`next if socket_const?(node.receiver)`), so these
            // offenses are report-only.
            ctx.report(&Self::META, offense_span, message);
            return;
        }

        let replacement = if name == b"freeze" {
            Edit::replace(node.span(), b"ENV".to_vec())
        } else {
            Edit::replace(offense_span, prefer.into_bytes())
        };
        let fix = Fix { applicability: Applicability::Safe, edits: vec![replacement] };
        ctx.report_with_fix(&Self::META, offense_span, message, fix);
    }
}
