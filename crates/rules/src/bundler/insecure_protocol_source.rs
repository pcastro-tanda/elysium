//! `Bundler/InsecureProtocolSource`, ported from RuboCop's
//! `lib/rubocop/cop/bundler/insecure_protocol_source.rb`.

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `MSG_HTTP_PROTOCOL`.
const MSG_HTTP_PROTOCOL: &str = "Use `https://rubygems.org` instead of `http://rubygems.org`.";

/// Checks for the use of a deprecated `source` argument in a Gemfile.
#[derive(Debug, Clone)]
pub struct InsecureProtocolSource {
    allow_http_protocol: bool,
}

impl Rule for InsecureProtocolSource {
    const META: RuleMeta = RuleMeta {
        name: "Bundler/InsecureProtocolSource",
        department: Department::Bundler,
        summary: "The source `:gemcutter`, `:rubygems` and `:rubyforge` are deprecated because HTTP requests are insecure. Please change your source to 'https://rubygems.org' if possible, or 'http://rubygems.org' if not.",
        explanation: "",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "AllowHttpProtocol",
            default: ConfigDefault::Bool(true),
            allowed: &[],
            doc: "Allow `source 'http://rubygems.org'` for safe autocorrection.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allow_http_protocol: options.bool("AllowHttpProtocol") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.receiver().is_some() || call.is_safe_navigation() {
            return;
        }
        if call.name().as_slice() != b"source" {
            return;
        }
        let Some(args) = call.arguments() else { return };
        let arguments = args.arguments();
        if arguments.len() != 1 {
            return;
        }
        let Some(arg) = arguments.first() else { return };

        let source = if let Some(sym) = arg.as_symbol_node() {
            let name = sym.unescaped();
            if name == b"gemcutter" || name == b"rubygems" || name == b"rubyforge" {
                String::from_utf8_lossy(name).into_owned()
            } else {
                return;
            }
        } else if let Some(s) = arg.as_string_node() {
            if s.unescaped() == b"http://rubygems.org" {
                "http://rubygems.org".to_string()
            } else {
                return;
            }
        } else {
            return;
        };

        let use_http_protocol = source == "http://rubygems.org";
        if self.allow_http_protocol && use_http_protocol {
            return;
        }

        let span = arg.span();
        let message = if use_http_protocol {
            MSG_HTTP_PROTOCOL.to_string()
        } else {
            format!(
                "The source `:{source}` is deprecated because HTTP requests are insecure. \
                 Please change your source to 'https://rubygems.org' if possible, or \
                 'http://rubygems.org' if not."
            )
        };

        let fix = Fix {
            applicability: Applicability::Safe,
            edits: vec![Edit::replace(span, b"'https://rubygems.org'".to_vec())],
        };
        ctx.report_with_fix(&Self::META, span, message, fix);
    }
}
