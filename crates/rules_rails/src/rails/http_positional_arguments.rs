//! `Rails/HttpPositionalArguments`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/http_positional_arguments.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::ext::{const_name, is_bare_or_toplevel_const};
use ruby_ast::{each_descendant, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use super::util::{plain_block_span, send_arguments, send_span, target_rails_version};

const KEYWORD_ARGS: &[&[u8]] = &[
    b"method", b"params", b"session", b"body", b"flash", b"xhr", b"as", b"headers", b"env", b"to",
];
const MINIMUM_TARGET_RAILS_VERSION: f64 = 5.0;

/// Use keyword arguments instead of positional arguments in http method calls.
#[derive(Debug, Clone)]
pub struct HttpPositionalArguments {
    supported: bool,
    /// `include Rack::Test::Methods` appears somewhere in the file.
    use_rack_test_methods: bool,
    /// Spans of `routes { }` / `draw { }` blocks entered so far.
    routing_blocks: Vec<Span>,
}

impl Rule for HttpPositionalArguments {
    const META: RuleMeta = RuleMeta {
        name: "Rails/HttpPositionalArguments",
        department: Department::Rails,
        summary: "Use keyword arguments instead of positional arguments in http method calls.",
        explanation: "Checks for the use of old-style attributes for HTTP request methods in \
                      tests, which Rails 5.0 deprecated (and 5.1 removed) in favour of keyword \
                      arguments.\n\n```ruby\n# bad\nget :new, { user_id: 1 }\n\n# good\nget :new, \
                      params: { user_id: 1 }\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            supported: target_rails_version(options) >= MINIMUM_TARGET_RAILS_VERSION,
            use_rack_test_methods: false,
            routing_blocks: Vec::new(),
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        self.routing_blocks.clear();
        self.use_rack_test_methods = false;
        if !self.supported {
            return;
        }
        let root = ctx.parsed().root();
        let mut found = false;
        each_descendant(&root, &mut |node| {
            if !found {
                found = node.as_call_node().is_some_and(|call| is_include_rack_test_methods(&call));
            }
        });
        self.use_rack_test_methods = found;
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        if !self.supported {
            return;
        }
        let Some(call) = node.as_call_node() else { return };
        if matches!(call.name().as_slice(), b"draw" | b"routes") {
            self.routing_blocks.extend(plain_block_span(&call));
        }
        if !matches!(
            call.name().as_slice(),
            b"get" | b"post" | b"put" | b"patch" | b"delete" | b"head"
        ) || call.receiver().is_some()
            || call.is_safe_navigation()
        {
            return;
        }
        let at = node.span().start;
        if self.use_rack_test_methods
            || self.routing_blocks.iter().any(|span| span.start <= at && at < span.end)
        {
            return;
        }
        let arguments = send_arguments(&call);
        if arguments.len() < 2 {
            return;
        }
        let data = &arguments[1..];
        if !needs_conversion(&data[0]) {
            return;
        }
        let verb = String::from_utf8_lossy(call.name().as_slice()).into_owned();
        let range = Span::new(data[0].span().start, data[data.len() - 1].span().end);
        let message = format!(
            "Use keyword arguments instead of positional arguments for http call: `{verb}`."
        );
        let call_span = send_span(&call);
        let action = String::from_utf8_lossy(ctx.text(arguments[0].span())).into_owned();
        let params = convert_hash_data(&data[0], "params", ctx);
        let session = if data.len() > 1 {
            convert_hash_data(&data[data.len() - 1], "session", ctx)
        } else {
            String::new()
        };
        let replacement = if call.opening_loc().is_some() {
            format!("{verb}({action}{params}{session})")
        } else {
            format!("{verb} {action}{params}{session}")
        };
        ctx.report_with_fix(
            &Self::META,
            range,
            message,
            Fix {
                applicability: Applicability::Safe,
                edits: vec![Edit::replace(call_span, replacement.into_bytes())],
            },
        );
    }
}

/// `(send nil? :include (const (const (const {nil? cbase} :Rack) :Test) :Methods))`.
fn is_include_rack_test_methods(call: &ruby_ast::node::CallNode<'_>) -> bool {
    if call.name().as_slice() != b"include"
        || call.receiver().is_some()
        || call.is_safe_navigation()
    {
        return false;
    }
    if call.block().is_some_and(|b| b.as_block_argument_node().is_some()) {
        return false;
    }
    let Some(arguments) = call.arguments() else { return false };
    let mut arguments = arguments.arguments().iter();
    let (Some(argument), None) = (arguments.next(), arguments.next()) else { return false };
    let Some(test) = named(&argument, b"Methods") else { return false };
    let Some(rack) = named(&test, b"Test") else { return false };
    is_bare_or_toplevel_const(&rack) && const_name(&rack).as_deref() == Some("Rack")
}

fn named<'pr>(node: &Node<'pr>, name: &[u8]) -> Option<Node<'pr>> {
    node.as_constant_path_node()
        .filter(|path| path.name().is_some_and(|n| n.as_slice() == name))
        .and_then(|path| path.parent())
}

fn hash_elements<'pr>(node: &Node<'pr>) -> Option<Vec<Node<'pr>>> {
    if let Some(hash) = node.as_hash_node() {
        Some(hash.elements().iter().collect())
    } else {
        node.as_keyword_hash_node().map(|hash| hash.elements().iter().collect())
    }
}

fn needs_conversion(data: &Node<'_>) -> bool {
    if data.as_forwarding_arguments_node().is_some() {
        return false;
    }
    let Some(elements) = hash_elements(data) else { return true };
    // `(hash (kwsplat _))` and `(hash (forwarded-kwrestarg))`.
    if elements.len() == 1 && elements[0].as_assoc_splat_node().is_some() {
        return false;
    }
    let pairs: Vec<_> = elements.iter().filter_map(Node::as_assoc_node).collect();
    !pairs.iter().any(|pair| {
        let key = pair.key();
        key.as_symbol_node().is_some_and(|sym| {
            KEYWORD_ARGS.contains(&sym.unescaped())
                || (sym.unescaped() == b"format" && pairs.len() == 1)
        })
    })
}

fn convert_hash_data(data: &Node<'_>, kind: &str, ctx: &Context<'_>) -> String {
    let hash_data = match hash_elements(data) {
        Some(elements) => {
            let pairs: Vec<String> = elements
                .iter()
                .filter(|e| e.as_assoc_node().is_some())
                .map(|e| String::from_utf8_lossy(ctx.text(e.span())).into_owned())
                .collect();
            if elements.is_empty() {
                return String::new();
            }
            format!("{{ {} }}", pairs.join(", "))
        }
        None => String::from_utf8_lossy(ctx.text(data.span())).into_owned(),
    };
    format!(", {kind}: {hash_data}")
}
