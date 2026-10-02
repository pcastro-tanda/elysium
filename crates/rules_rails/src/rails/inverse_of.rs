//! `Rails/InverseOf`, ported from rubocop-rails's
//! `lib/rubocop/cop/rails/inverse_of.rb`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const SPECIFY_MSG: &str = "Specify an `:inverse_of` option.";
const NIL_MSG: &str =
    "You specified `inverse_of: nil`, you probably meant to use `inverse_of: false`.";

/// An element of an options hash, reduced to what the cop distinguishes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opt {
    Conditions,
    Through,
    Polymorphic,
    As,
    ForeignKey,
    InverseOf,
    /// `(pair (sym :inverse_of) nil)`.
    InverseOfNil,
    /// A `**splat` (`kwsplat_type?`).
    Splat,
    Other,
}

/// The first block parameter of a `with_options` block, as far as
/// `same_context_in_with_options?` can tell.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Param {
    None,
    Name(Vec<u8>),
    /// A parameter whose first child is not a plain name (destructuring).
    Other,
}

/// A `with_options ... do ... end` call being inside of.
#[derive(Debug, Clone)]
struct WithOptions {
    call: Span,
    block: Span,
    param: Param,
    options: Vec<Opt>,
    /// Whether one of the call's arguments is itself a `block` node.
    scope: bool,
}

/// Checks for associations where the inverse cannot be determined
/// automatically.
#[derive(Debug, Clone)]
pub struct InverseOf {
    ignore_scopes: bool,
    rails_5_2: bool,
    with_options: Vec<WithOptions>,
}

impl Rule for InverseOf {
    const META: RuleMeta = RuleMeta {
        name: "Rails/InverseOf",
        department: Department::Rails,
        summary: "Checks for associations where the inverse cannot be determined automatically.",
        explanation: "Looks for has_(one|many) and belongs_to associations where Active Record \
                      can't automatically determine the inverse association because of a scope \
                      or the options used. Using the blog with order scope example below, \
                      traversing the a Blog's association in both directions with \
                      `blog.posts.first.blog` would cause the `blog` to be loaded from the \
                      database twice.\n\n`:inverse_of` must be manually specified for Active \
                      Record to use the associated object in memory, or set to `false` to \
                      opt-out. Note that setting `nil` does not stop Active Record from trying \
                      to determine the inverse automatically, and is not considered a valid \
                      value for this.\n\n```ruby\n# bad\nclass Blog < ApplicationRecord\n  \
                      has_many :posts, -> { order(published_at: :desc) }\nend\n\n# good\nclass \
                      Blog < ApplicationRecord\n  has_many(:posts,\n           -> { \
                      order(published_at: :desc) },\n           inverse_of: :blog)\nend\n```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::CallNode],
        config: &[ConfigOption {
            name: "IgnoreScopes",
            default: ConfigDefault::Bool(false),
            allowed: &[],
            doc: "Do not require `:inverse_of` for associations with a scope.",
        }],
        blind_spots: "Without `AllCops/TargetRailsVersion` the Rails version is taken to be \
                      5.0; RuboCop reads `railties` from the project's `Gemfile.lock` first.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self {
            ignore_scopes: options.bool("IgnoreScopes"),
            rails_5_2: options.target_rails_version() >= 5.2,
            with_options: Vec::new(),
        })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        match call.name().as_slice() {
            b"with_options" => self.enter_with_options(node),
            b"has_many" | b"has_one" | b"belongs_to" => self.check_association(node, ctx),
            _ => {}
        }
    }

    fn leave(&mut self, node: &Node<'_>, _ctx: &mut Context<'_>) {
        if self.with_options.last().is_some_and(|entry| entry.call == node.span()) {
            self.with_options.pop();
        }
    }
}

impl InverseOf {
    /// Remember a `with_options` call (no receiver, literal block with
    /// ordinary parameters: the parser's `block` type).
    fn enter_with_options(&mut self, node: &Node<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.receiver().is_some() {
            return;
        }
        let Some(block) = call.block() else { return };
        let Some(block_node) = block.as_block_node() else { return };
        let param = match block_node.parameters() {
            None => Param::None,
            Some(parameters) => match parameters.kind() {
                NodeKind::BlockParametersNode => {
                    first_parameter(&parameters).unwrap_or(Param::None)
                }
                // `numblock`/`itblock`, not a `block`.
                _ => return,
            },
        };
        let arguments: Vec<Node<'_>> =
            call.arguments().map_or_else(Vec::new, |a| a.arguments().iter().collect());
        self.with_options.push(WithOptions {
            call: node.span(),
            block: block.span(),
            param,
            options: options_of(&arguments),
            scope: arguments.iter().any(is_block_type),
        });
    }

    fn check_association(&self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return;
        }
        // `(send $_ {:has_many :has_one :belongs_to} _ $...)`: at least one
        // argument, a `&block` argument counting as the last.
        let Some(arguments) = call.arguments() else { return };
        let all: Vec<Node<'_>> = arguments.arguments().iter().collect();
        let Some((_, rest)) = all.split_first() else { return };

        let receiver = call.receiver();
        let enclosing: Vec<&WithOptions> = self
            .with_options
            .iter()
            .filter(|entry| {
                entry.block.start <= node.span().start
                    && node.span().end <= entry.block.end
                    && same_context(&entry.param, receiver.as_ref())
            })
            .collect();

        let mut options = options_of(rest);
        let mut scope = rest.iter().any(is_block_type);
        for entry in enclosing {
            options.extend_from_slice(&entry.options);
            scope |= entry.scope;
        }

        if options.iter().any(|o| matches!(o, Opt::Through | Opt::Polymorphic)) {
            return;
        }
        let requiring = options.iter().any(|o| {
            matches!(o, Opt::Conditions | Opt::ForeignKey) || (!self.rails_5_2 && *o == Opt::As)
        });
        if !((!self.ignore_scopes && scope) || requiring) {
            return;
        }
        if options.contains(&Opt::InverseOf) {
            return;
        }
        let nil_option = options.contains(&Opt::InverseOfNil);
        if options.contains(&Opt::Splat) && !nil_option {
            return;
        }
        let Some(selector) = call.message_loc() else { return };
        ctx.report(&Self::META, selector.span(), if nil_option { NIL_MSG } else { SPECIFY_MSG });
    }
}

/// `same_context_in_with_options?(block.first_argument, recv)`.
fn same_context(param: &Param, receiver: Option<&Node<'_>>) -> bool {
    match (param, receiver) {
        (Param::None, None) => true,
        (Param::Name(name), Some(receiver)) => receiver
            .as_local_variable_read_node()
            .is_some_and(|read| read.name().as_slice() == name.as_slice()),
        _ => false,
    }
}

/// `block_type?`: a literal block, or a lambda, with ordinary parameters.
fn is_block_type(node: &Node<'_>) -> bool {
    let ordinary = |parameters: Option<Node<'_>>| {
        parameters.is_none_or(|p| {
            !matches!(p.kind(), NodeKind::NumberedParametersNode | NodeKind::ItParametersNode)
        })
    };
    match node.kind() {
        NodeKind::LambdaNode => {
            node.as_lambda_node().is_some_and(|lambda| ordinary(lambda.parameters()))
        }
        NodeKind::CallNode => node
            .as_call_node()
            .and_then(|call| call.block())
            .and_then(|block| block.as_block_node().map(|b| ordinary(b.parameters())))
            .unwrap_or(false),
        _ => false,
    }
}

/// `options_from_argument` over every argument: the elements of each hash.
fn options_of(arguments: &[Node<'_>]) -> Vec<Opt> {
    let mut options = Vec::new();
    for argument in arguments {
        let elements: Vec<Node<'_>> = match argument.kind() {
            NodeKind::HashNode => {
                argument.as_hash_node().map_or_else(Vec::new, |h| h.elements().iter().collect())
            }
            NodeKind::KeywordHashNode => argument
                .as_keyword_hash_node()
                .map_or_else(Vec::new, |h| h.elements().iter().collect()),
            _ => {
                options.push(Opt::Other);
                continue;
            }
        };
        options.extend(elements.iter().map(classify));
    }
    options
}

fn classify(element: &Node<'_>) -> Opt {
    if element.kind() == NodeKind::AssocSplatNode {
        return Opt::Splat;
    }
    let Some(pair) = element.as_assoc_node() else { return Opt::Other };
    let key = pair.key();
    let Some(key) = key.as_symbol_node() else { return Opt::Other };
    let nil_value = pair.value().kind() == NodeKind::NilNode;
    match key.unescaped() {
        b"inverse_of" if nil_value => Opt::InverseOfNil,
        _ if nil_value => Opt::Other,
        b"conditions" => Opt::Conditions,
        b"through" => Opt::Through,
        b"polymorphic" => Opt::Polymorphic,
        b"as" => Opt::As,
        b"foreign_key" => Opt::ForeignKey,
        b"inverse_of" => Opt::InverseOf,
        _ => Opt::Other,
    }
}

/// The first parameter of a block's parameter list, in source order.
fn first_parameter(block_parameters: &Node<'_>) -> Option<Param> {
    let parameters = block_parameters.as_block_parameters_node()?.parameters()?;
    let named = |node: &Node<'_>| match node.kind() {
        NodeKind::RequiredParameterNode => node
            .as_required_parameter_node()
            .map_or(Param::Other, |p| Param::Name(p.name().as_slice().to_vec())),
        NodeKind::OptionalParameterNode => node
            .as_optional_parameter_node()
            .map_or(Param::Other, |p| Param::Name(p.name().as_slice().to_vec())),
        NodeKind::RestParameterNode => node
            .as_rest_parameter_node()
            .and_then(|p| p.name())
            .map_or(Param::Other, |n| Param::Name(n.as_slice().to_vec())),
        NodeKind::RequiredKeywordParameterNode => node
            .as_required_keyword_parameter_node()
            .map_or(Param::Other, |p| Param::Name(p.name().as_slice().to_vec())),
        NodeKind::OptionalKeywordParameterNode => node
            .as_optional_keyword_parameter_node()
            .map_or(Param::Other, |p| Param::Name(p.name().as_slice().to_vec())),
        _ => Param::Other,
    };
    parameters
        .requireds()
        .iter()
        .next()
        .or_else(|| parameters.optionals().iter().next())
        .or_else(|| parameters.rest())
        .or_else(|| parameters.posts().iter().next())
        .or_else(|| parameters.keywords().iter().next())
        .or_else(|| parameters.keyword_rest())
        .map(|node| named(&node))
}
