//! `Lint/DeprecatedOpenSSLConstant`, ported from RuboCop's
//! `lib/rubocop/cop/lint/deprecated_open_ssl_constant.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Upstream's `NO_ARG_ALGORITHM`: ciphers named directly by their algorithm,
/// with no explicit block size or mode suffix.
const NO_ARG_ALGORITHM: [&str; 4] = ["BF", "DES", "IDEA", "RC4"];

/// Don't use algorithm constants for `OpenSSL::Cipher` and `OpenSSL::Digest`.
#[derive(Debug, Clone)]
pub struct DeprecatedOpenSSLConstant;

/// `rubocop-ast`'s `Node#const_name`, but only the node's own (rightmost)
/// segment -- upstream's `digest_const?`/`algorithm_const` matchers key off
/// individual `const` node names (`node.loc.name.source` / the symbol in a
/// `{:Cipher :Digest}` pattern arm), not the whole qualified path.
fn own_const_name<'pr>(node: &Node<'pr>) -> Option<&'pr [u8]> {
    if let Some(read) = node.as_constant_read_node() {
        Some(read.name().as_slice())
    } else if let Some(path) = node.as_constant_path_node() {
        path.name().map(|n| n.as_slice())
    } else {
        None
    }
}

/// `rubocop-ast`'s `Node#variable?`: `ivar`/`gvar`/`cvar`/`lvar` reads.
fn is_variable_read(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::LocalVariableReadNode
            | NodeKind::InstanceVariableReadNode
            | NodeKind::ClassVariableReadNode
            | NodeKind::GlobalVariableReadNode
    )
}

/// `rubocop-ast`'s `Node#const_type?`.
fn is_const(node: &Node<'_>) -> bool {
    matches!(node.kind(), NodeKind::ConstantReadNode | NodeKind::ConstantPathNode)
}

/// `String#split('-')` without a limit: drops a trailing run of empty
/// fields but keeps interior ones, and an empty input splits to nothing.
fn ruby_dash_split(s: &str) -> Vec<String> {
    let mut parts: Vec<&str> = s.split('-').collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts.into_iter().map(str::to_string).collect()
}

/// `name.scan(/.{3}/).join('-')`: groups of exactly 3 bytes, dropping any
/// shorter remainder -- e.g. `"AES128"` -> `"AES-128"`.
fn scan_triples(name: &str) -> String {
    let bytes = name.as_bytes();
    let mut parts = Vec::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        parts.push(&name[i..i + 3]);
        i += 3;
    }
    parts.join("-")
}

/// `sanitize_arguments`: each argument's string value (or raw source for
/// non-string literals), stripped of `:`/`'`, then split on `-`.
fn sanitize_arguments(ctx: &Context<'_>, args: &[Node<'_>]) -> Vec<String> {
    let mut out = Vec::new();
    for arg in args {
        let text = if let Some(s) = arg.as_string_node() {
            String::from_utf8_lossy(s.unescaped()).into_owned()
        } else {
            String::from_utf8_lossy(ctx.text(arg.span())).into_owned()
        };
        let cleaned: String = text.chars().filter(|&c| c != ':' && c != '\'').collect();
        out.extend(ruby_dash_split(&cleaned));
    }
    out
}

/// `build_cipher_arguments`.
fn build_cipher_arguments(
    ctx: &Context<'_>,
    algorithm_name: &str,
    args: &[Node<'_>],
    no_arguments: bool,
) -> String {
    let algorithm_parts = ruby_dash_split(&algorithm_name.to_lowercase());
    let size_and_mode: Vec<String> =
        sanitize_arguments(ctx, args).into_iter().map(|s| s.to_lowercase()).collect();

    let first = algorithm_parts.first().cloned().unwrap_or_default();
    if no_arguments && NO_ARG_ALGORITHM.contains(&first.to_uppercase().as_str()) {
        return format!("'{first}'");
    }

    let mode = size_and_mode.is_empty().then(|| "cbc".to_string());
    let mut combined = algorithm_parts;
    combined.extend(size_and_mode);
    combined.extend(mode);
    combined.truncate(3);
    format!("'{}'", combined.join("-"))
}

impl Rule for DeprecatedOpenSSLConstant {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DeprecatedOpenSSLConstant",
        department: Department::Lint,
        summary: "Don't use algorithm constants for `OpenSSL::Cipher` and `OpenSSL::Digest`.",
        explanation: "\
Algorithmic constants for `OpenSSL::Cipher` and `OpenSSL::Digest`
deprecated since OpenSSL version 2.2.0. Prefer passing a string
instead.

```ruby
# bad
OpenSSL::Cipher::AES.new(128, :GCM)

# good
OpenSSL::Cipher.new('aes-128-gcm')

# bad
OpenSSL::Digest::SHA256.new

# good
OpenSSL::Digest.new('SHA256')

# bad
OpenSSL::Digest::SHA256.digest('foo')

# good
OpenSSL::Digest.digest('SHA256', 'foo')
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "\
The argument skip-list (`arg.variable? || arg.call_type? || arg.const_type?`)
relies on Prism's own parse-time local-variable tracking to tell a bare
identifier read (`lvar`) apart from a zero-arg method call, exactly like
whitequark's parser does upstream; no discrepancy is expected.",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return;
        }
        let method_bytes = call.name().as_slice();
        if method_bytes != b"new" && method_bytes != b"digest" {
            return;
        }

        let args: Vec<Node<'_>> =
            call.arguments().map_or_else(Vec::new, |a| a.arguments().iter().collect());
        if args
            .iter()
            .any(|arg| is_variable_read(arg) || arg.kind() == NodeKind::CallNode || is_const(arg))
        {
            return;
        }

        let Some(receiver) = call.receiver() else { return };

        // `digest_const?`: the algorithm-level constant is itself literally
        // named `Digest` (e.g. `OpenSSL::Digest::Digest`), which is not an
        // algorithm at all.
        if own_const_name(&receiver).is_some_and(|n| n == b"Digest") {
            return;
        }

        // `algorithm_const`: `receiver` is `OpenSSL::{Cipher,Digest}::<algo>`,
        // optionally with a leading `::` in front of `OpenSSL`.
        let Some(algo_path) = receiver.as_constant_path_node() else { return };
        if algo_path.name().is_none() {
            return;
        }
        let algo_name_text = String::from_utf8_lossy(ctx.text(algo_path.name_loc().span()));
        let algo_name_text = algo_name_text.as_ref();

        let Some(mid) = algo_path.parent() else { return };
        let Some(mid_name) = own_const_name(&mid) else { return };
        if mid_name != b"Cipher" && mid_name != b"Digest" {
            return;
        }
        let Some(mid_path) = mid.as_constant_path_node() else { return };
        let Some(inner) = mid_path.parent() else { return };
        let Some(inner_name) = own_const_name(&inner) else { return };
        if inner_name != b"OpenSSL" {
            return;
        }
        if inner.as_constant_path_node().is_some_and(|p| p.parent().is_some()) {
            return;
        }

        // `openssl_class(algorithm_constant)`: the raw source of the
        // `OpenSSL::Cipher`/`OpenSSL::Digest` segment, compared verbatim
        // (a leading `::` makes this not equal `"OpenSSL::Cipher"`, exactly
        // as upstream's own string comparison would).
        let parent_constant = String::from_utf8_lossy(ctx.text(mid.span())).into_owned();
        let is_cipher_class = parent_constant == "OpenSSL::Cipher";

        let receiver_source = ctx.text(receiver.span());
        let replacement_args = if receiver_source == b"OpenSSL::Cipher::Cipher" {
            match args.first() {
                Some(first) => String::from_utf8_lossy(ctx.text(first.span())).into_owned(),
                None => String::new(),
            }
        } else {
            let algorithm_name = if is_cipher_class && !NO_ARG_ALGORITHM.contains(&algo_name_text) {
                scan_triples(algo_name_text)
            } else {
                algo_name_text.to_string()
            };

            if is_cipher_class {
                build_cipher_arguments(ctx, &algorithm_name, &args, args.is_empty())
            } else {
                let mut parts = vec![format!("'{algorithm_name}'")];
                parts.extend(
                    args.iter().map(|a| String::from_utf8_lossy(ctx.text(a.span())).into_owned()),
                );
                parts.join(", ")
            }
        };

        let Some(message_loc) = call.message_loc() else { return };
        let method_text = String::from_utf8_lossy(ctx.text(message_loc.span())).into_owned();
        let original = String::from_utf8_lossy(ctx.text(node.span())).into_owned();

        let message = format!(
            "Use `{parent_constant}.{method_text}({replacement_args})` instead of `{original}`."
        );

        let delimiter_span = algo_path.delimiter_loc().span();
        let replace_span = Span::new(delimiter_span.start, node.span().end);
        let replacement = format!(".{method_text}({replacement_args})");

        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(replace_span, replacement.into_bytes())],
        };

        ctx.report_with_fix(&Self::META, node.span(), message, fix);
    }
}
