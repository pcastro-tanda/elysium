//! `Sorbet/RedundantTLetForLiteral`, ported from rubocop-sorbet's
//! `lib/rubocop/cop/sorbet/redundant_t_let_for_literal.rb` (with its
//! `ConstantScope`, `TargetSorbetVersion` and `TLetCorrection` mixins, and
//! `RBSParser.rbs_annotation_after`).

use std::sync::LazyLock;

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, GemVersion, OptionError, Rule,
    RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::ext::is_heredoc;
use ruby_ast::node::CallNode;
use ruby_ast::{each_descendant, for_each_child, LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

macro_rules! regex {
    ($pattern:expr) => {{
        static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new($pattern).expect("valid regex"));
        &*RE
    }};
}

/// `minimum_target_sorbet_static_version`.
const MINIMUM_SORBET_STATIC_VERSION: &str = "0.6.13304";

/// Checks for redundant `T.let` declarations and trailing RBS annotations on
/// constants whose literal values have types Sorbet can infer automatically.
#[derive(Debug, Clone)]
pub struct RedundantTLetForLiteral {
    /// `enabled_for_sorbet_static_version?`: `sorbet-static` is locked and at
    /// least the minimum version.
    enabled_for_sorbet_static_version: bool,
}

impl Rule for RedundantTLetForLiteral {
    const META: RuleMeta = RuleMeta {
        name: "Sorbet/RedundantTLetForLiteral",
        department: Department::Sorbet,
        summary: "Checks for redundant `T.let` declarations and trailing RBS annotations on constants whose literal values have types Sorbet can infer automatically.",
        explanation: "Checks for redundant `T.let` declarations and trailing RBS annotations \
                      where the assigned value is a literal whose type Sorbet can infer \
                      automatically.\n\nSimple literals (strings, symbols, integers, floats, \
                      regexps) infer as their own class. Regexp literals are the only simple \
                      literals whose inference survives a `.freeze` call (Sorbet 0.6.13304+), \
                      so `T.let(/foo/.freeze, Regexp)` is also redundant; other frozen simple \
                      literals (e.g. `\"hello\".freeze`) are not inferred and still need \
                      `T.let`.\n\nArray literals of simple literals are also inferred:\n\n\
                      * A frozen array (`[...].freeze`) infers as a fixed-size tuple, which is \
                      a subtype of the annotated `T::Array`, so the annotation is redundant.\n\
                      * An unfrozen array infers as `T::Array[<element type>]`. It is only \
                      flagged when that inferred type matches the annotation exactly, to avoid \
                      silently widening (e.g. `[\"a\", nil]` infers a nilable element).\n\n\
                      Hashes are excluded: Sorbet infers hash literals as `T.untyped`, so the \
                      annotation is required.\n\n```ruby\n# bad\nMAX_RETRIES = T.let(3, \
                      Integer)\nGREETING = T.let(\"hello\", String)\nSHELLS = \
                      T.let([:bash, :zsh].freeze, T::Array[Symbol])\nRBS_GREETING = \"hello\" \
                      #: String\n\n# good\nMAX_RETRIES = 3\nGREETING = \"hello\"\nSHELLS = \
                      [:bash, :zsh].freeze\nRBS_GREETING = \"hello\"\n```",
        enabled_by_default: false,
        severity: Severity::Convention,
        fix: FixAvailability::Unsafe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::ConstantWriteNode, NodeKind::ConstantPathWriteNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let minimum = GemVersion::parse(MINIMUM_SORBET_STATIC_VERSION);
        let enabled = match (options.gem_version("sorbet-static"), minimum) {
            (Some(version), Some(minimum)) => minimum <= version,
            _ => false,
        };
        Ok(Self { enabled_for_sorbet_static_version: enabled })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let value = match node.kind() {
            NodeKind::ConstantWriteNode => node.as_constant_write_node().map(|w| w.value()),
            NodeKind::ConstantPathWriteNode => {
                node.as_constant_path_write_node().map(|w| w.value())
            }
            _ => None,
        };
        let Some(value) = value else { return };
        self.on_casgn(node, &value, ctx);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Literal {
    String,
    Symbol,
    Integer,
    Float,
    Regexp,
}

impl Literal {
    fn class(self) -> &'static str {
        match self {
            Self::String => "String",
            Self::Symbol => "Symbol",
            Self::Integer => "Integer",
            Self::Float => "Float",
            Self::Regexp => "Regexp",
        }
    }
}

/// `LITERAL_TYPE_TO_CLASS[node.type]`.
fn literal_class(node: &Node<'_>) -> Option<Literal> {
    match node.kind() {
        NodeKind::InterpolatedStringNode | NodeKind::StringNode | NodeKind::SourceFileNode => {
            Some(Literal::String)
        }
        NodeKind::InterpolatedSymbolNode | NodeKind::SymbolNode => Some(Literal::Symbol),
        NodeKind::FloatNode => Some(Literal::Float),
        NodeKind::IntegerNode | NodeKind::SourceLineNode => Some(Literal::Integer),
        NodeKind::RegularExpressionNode | NodeKind::InterpolatedRegularExpressionNode => {
            Some(Literal::Regexp)
        }
        _ => None,
    }
}

fn is_regexp(node: &Node<'_>) -> bool {
    matches!(
        node.kind(),
        NodeKind::RegularExpressionNode | NodeKind::InterpolatedRegularExpressionNode
    )
}

/// `ELEMENT_TYPE_TO_CLASS[child.type]`.
fn element_class(node: &Node<'_>) -> Option<&'static str> {
    match node.kind() {
        NodeKind::InterpolatedStringNode | NodeKind::StringNode | NodeKind::SourceFileNode => {
            Some("String")
        }
        NodeKind::InterpolatedSymbolNode | NodeKind::SymbolNode => Some("Symbol"),
        NodeKind::FloatNode => Some("Float"),
        NodeKind::IntegerNode | NodeKind::SourceLineNode => Some("Integer"),
        _ => None,
    }
}

/// `ARRAY_ELEMENT_TYPES.include?(child.type)`.
fn is_array_element_type(node: &Node<'_>) -> bool {
    element_class(node).is_some()
        || matches!(node.kind(), NodeKind::TrueNode | NodeKind::FalseNode | NodeKind::NilNode)
}

impl RedundantTLetForLiteral {
    fn on_casgn(&self, node: &Node<'_>, value: &Node<'_>, ctx: &mut Context<'_>) {
        let root = ctx.parsed().root();
        let mut path = Vec::new();
        if !find_path(&root, node, &mut path) || !statically_scoped(&path) {
            return;
        }

        self.check_t_let(value, ctx);
        self.check_rbs_annotation(node, value, ctx);
    }

    fn check_t_let(&self, value: &Node<'_>, ctx: &mut Context<'_>) {
        // `(casgn _ _ (send (const {nil? cbase} :T) :let $_ $_))`
        let Some(call) = value.as_call_node() else { return };
        let Some((first, second)) = t_let_arguments(&call) else { return };

        // `t_let_with_literal_and_class?`:
        // `${literal? (send (regexp ...) :freeze)} (const nil? $_)`
        let freeze_without_arguments =
            first.as_call_node().is_none_or(|first_call| argument_list(&first_call).is_empty());
        if let Some(class_name) = second.as_constant_read_node().map(|constant| constant.name()) {
            if let Some(literal_node) =
                self.inferable_literal_node(&first).filter(|_| freeze_without_arguments)
            {
                if literal_class(&literal_node)
                    .is_some_and(|literal| literal.class().as_bytes() == class_name.as_slice())
                {
                    let type_name = String::from_utf8_lossy(class_name.as_slice()).into_owned();
                    register_offense(ctx, value, &first, "`T.let`", &type_name);
                }
            }
        }

        // `t_let_with_array?`: `${array (send array :freeze)} $_`
        if self.enabled_for_sorbet_static_version {
            let (array_node, frozen) = match first.as_call_node() {
                Some(freeze) if is_send(&freeze) && freeze.name().as_slice() == b"freeze" => {
                    if !argument_list(&freeze).is_empty() {
                        return;
                    }
                    let Some(receiver) = freeze.receiver() else { return };
                    (receiver, true)
                }
                _ => (first, false),
            };
            if array_node.as_array_node().is_some()
                && inferable_array(&array_node)
                && redundant_array_annotation(ctx, &array_node, &second, frozen)
            {
                register_offense(ctx, value, &first, "`T.let`", "Array");
            }
        }
    }

    /// Returns the underlying literal when its inference is supported by the
    /// target Sorbet version. Bare literals return themselves. A frozen value
    /// returns its receiver only for regexps, whose inference survives
    /// `.freeze` on supported targets.
    fn inferable_literal_node<'pr>(&self, value_node: &Node<'pr>) -> Option<Node<'pr>> {
        let Some(call) = value_node.as_call_node().filter(is_send) else {
            return Some(*value_node);
        };
        if call.name().as_slice() != b"freeze" {
            return None;
        }
        let receiver = call.receiver().filter(is_regexp)?;
        if !self.enabled_for_sorbet_static_version {
            return None;
        }
        Some(receiver)
    }

    fn check_rbs_annotation(&self, node: &Node<'_>, value_node: &Node<'_>, ctx: &mut Context<'_>) {
        let Some((comment, annotation)) = rbs_annotation_after(ctx, node.span()) else { return };
        let annotated_type = normalize(&annotation);

        if let Some(literal_node) = self.inferable_literal_node(value_node) {
            if let Some(inferred) = literal_class(&literal_node) {
                let inferred_type = inferred.class();
                if annotated_type.strip_prefix("::").unwrap_or(&annotated_type) != inferred_type {
                    return;
                }
                register_rbs_offense(ctx, node, comment, inferred_type);
                return;
            }
        }

        if !self.enabled_for_sorbet_static_version {
            return;
        }
        let frozen = value_node.as_call_node().is_some_and(|call| {
            is_send(&call) && call.name().as_slice() == b"freeze" && call.receiver().is_some()
        });
        let array_node = if frozen {
            value_node.as_call_node().and_then(|call| call.receiver())
        } else {
            Some(*value_node)
        };
        let Some(array_node) = array_node else { return };
        if array_node.as_array_node().is_none()
            || !inferable_array(&array_node)
            || !redundant_rbs_array_annotation(&array_node, &annotated_type, frozen)
        {
            return;
        }
        register_rbs_offense(ctx, node, comment, "Array");
    }
}

fn register_offense(
    ctx: &mut Context<'_>,
    t_let_node: &Node<'_>,
    value_node: &Node<'_>,
    annotation: &str,
    type_name: &str,
) {
    let message = format!(
        "Redundant {annotation} for {type_name} literal. Sorbet can infer this type automatically."
    );
    let fix = replace_t_let(ctx, t_let_node, value_node);
    ctx.report_with_fix(
        &RedundantTLetForLiteral::META,
        t_let_node.span(),
        message,
        Fix { applicability: Applicability::Unsafe, edits: vec![fix] },
    );
}

fn register_rbs_offense(ctx: &mut Context<'_>, node: &Node<'_>, comment: Span, type_name: &str) {
    let message = format!(
        "Redundant RBS annotation for {type_name} literal. Sorbet can infer this type automatically."
    );
    let range = Span::new(node.span().end, comment.end);
    ctx.report_with_fix(
        &RedundantTLetForLiteral::META,
        comment,
        message,
        Fix { applicability: Applicability::Unsafe, edits: vec![Edit::delete(range)] },
    );
}

/// `RBSParser.rbs_annotation_after`: the trailing `#:` comment following the
/// expression on its last line, as `(comment span, annotation)`.
fn rbs_annotation_after(ctx: &Context<'_>, span: Span) -> Option<(Span, String)> {
    let last_line = ctx.last_line(span);
    let source = ctx.source().bytes();
    let comment = ctx.comments().iter().find(|comment| {
        if comment.line != last_line || comment.span.start < span.end {
            return false;
        }
        source[span.end as usize..comment.span.start as usize]
            .iter()
            .all(|byte| matches!(byte, b' ' | b'\t'))
    })?;
    let text = ctx.text(comment.span);
    let annotation = text.strip_prefix(b"#:")?;
    let annotation = String::from_utf8_lossy(annotation.trim_ascii()).into_owned();
    if annotation.is_empty() {
        return None;
    }
    Some((comment.span, annotation))
}

/// An array literal is inferable only when every element is one of the simple
/// literals Sorbet reflects into the element type (or a nested inferable
/// array). Empty arrays are excluded.
fn inferable_array(node: &Node<'_>) -> bool {
    let Some(array) = node.as_array_node() else { return false };
    let elements: Vec<Node<'_>> = array.elements().iter().collect();
    if elements.is_empty() {
        return false;
    }
    elements.iter().all(|child| {
        if child.as_array_node().is_some() {
            inferable_array(child)
        } else {
            is_array_element_type(child)
        }
    })
}

/// Whether Sorbet's inferred array type makes the annotation redundant.
fn redundant_array_annotation(
    ctx: &Context<'_>,
    array_node: &Node<'_>,
    type_node: &Node<'_>,
    frozen: bool,
) -> bool {
    if frozen {
        return is_t_array_type(ctx, type_node);
    }
    let normalized = normalize(&String::from_utf8_lossy(ctx.text(type_node.span())));
    inferred_array_type(array_node)
        .is_some_and(|inferred| inferred == normalized.strip_prefix("::").unwrap_or(&normalized))
}

/// `T::Array[...]` (with or without a leading `::`).
fn is_t_array_type(ctx: &Context<'_>, node: &Node<'_>) -> bool {
    let Some(call) = node.as_call_node().filter(is_send) else { return false };
    if call.name().as_slice() != b"[]" {
        return false;
    }
    call.receiver().is_some_and(|receiver| {
        let text = ctx.text(receiver.span());
        text.strip_prefix(b"::").unwrap_or(text) == b"T::Array"
    })
}

fn redundant_rbs_array_annotation(array_node: &Node<'_>, type_name: &str, frozen: bool) -> bool {
    let rbs_type = type_name.strip_prefix("::").unwrap_or(type_name);
    if frozen {
        // `/\AArray\[(?:.+)\]\z/`
        return rbs_type.starts_with("Array[") && rbs_type.ends_with(']') && rbs_type.len() > 7;
    }
    inferred_array_type(array_node)
        .is_some_and(|inferred| inferred.strip_prefix("T::").unwrap_or(&inferred) == rbs_type)
}

/// The type Sorbet infers for an unfrozen array literal, or `None` when the
/// element classes are not uniform.
fn inferred_array_type(node: &Node<'_>) -> Option<String> {
    let array = node.as_array_node()?;
    let classes: Vec<Option<&str>> =
        array.elements().iter().map(|child| element_class(&child)).collect();
    let first = (*classes.first()?)?;
    if !classes.iter().all(|class| *class == Some(first)) {
        return None;
    }
    Some(format!("T::Array[{first}]"))
}

/// Strips whitespace and any trailing comma before a closing delimiter.
fn normalize(source: &str) -> String {
    let squeezed = regex!(r"[ \t\r\n\x0c\x0b]+").replace_all(source, "");
    regex!(r",([)\]}])").replace_all(&squeezed, "$1").into_owned()
}

fn is_const_t(node: &Node<'_>) -> bool {
    // `(const {nil? cbase} :T)`
    if let Some(constant) = node.as_constant_read_node() {
        return constant.name().as_slice() == b"T";
    }
    node.as_constant_path_node().is_some_and(|path| {
        path.parent().is_none() && path.name().is_some_and(|name| name.as_slice() == b"T")
    })
}

/// `send` arguments as whitequark sees them: a `&blk` is a `block_pass` child.
fn argument_list<'pr>(call: &CallNode<'pr>) -> Vec<Node<'pr>> {
    let mut args: Vec<Node<'pr>> =
        call.arguments().map(|args| args.arguments().iter().collect()).unwrap_or_default();
    if let Some(block) = call.block().filter(|block| block.as_block_argument_node().is_some()) {
        args.push(block);
    }
    args
}

/// A whitequark `send`: neither `csend` nor a call that carries a literal
/// block (which becomes a `block` node).
fn is_send(call: &CallNode<'_>) -> bool {
    !call.is_safe_navigation() && call.block().is_none_or(|b| b.as_block_node().is_none())
}

/// `(send (const {nil? cbase} :T) :let $_ $_)`.
fn t_let_arguments<'pr>(call: &CallNode<'pr>) -> Option<(Node<'pr>, Node<'pr>)> {
    if call.name().as_slice() != b"let" || !is_send(call) || !is_const_t(&call.receiver()?) {
        return None;
    }
    match argument_list(call).as_slice() {
        [first, second] => Some((*first, *second)),
        _ => None,
    }
}

/// The Prism ancestors (outermost first) of `target`.
fn find_path<'pr>(node: &Node<'pr>, target: &Node<'_>, path: &mut Vec<Node<'pr>>) -> bool {
    if node.span() == target.span() && node.kind() == target.kind() {
        return true;
    }
    let (span, wanted) = (node.span(), target.span());
    if span.start > wanted.start || span.end < wanted.end {
        return false;
    }
    path.push(*node);
    let mut found = false;
    for_each_child(node, |child| {
        if !found && find_path(child, target, path) {
            found = true;
        }
    });
    if !found {
        path.pop();
    }
    found
}

/// `ConstantScope#statically_scoped?`, over whitequark's ancestors: a
/// `StatementsNode` is a `begin` only with several statements and no
/// `ParenthesesNode` around it (the parentheses are the `begin` then).
fn statically_scoped(path: &[Node<'_>]) -> bool {
    #[derive(Clone, Copy, PartialEq)]
    enum Ancestor {
        Begin,
        Scope,
        Other,
    }
    let mut chain = Vec::with_capacity(2);
    for index in (0..path.len()).rev() {
        if chain.len() == 2 {
            break;
        }
        let ancestor = &path[index];
        match ancestor.kind() {
            NodeKind::ArgumentsNode | NodeKind::ProgramNode => {}
            NodeKind::StatementsNode => {
                let in_parentheses = index
                    .checked_sub(1)
                    .is_some_and(|outer| path[outer].kind() == NodeKind::ParenthesesNode);
                let several = ancestor
                    .as_statements_node()
                    .is_some_and(|statements| statements.body().iter().count() >= 2);
                if several && !in_parentheses {
                    chain.push(Ancestor::Begin);
                }
            }
            NodeKind::ParenthesesNode => chain.push(Ancestor::Begin),
            NodeKind::ClassNode | NodeKind::ModuleNode | NodeKind::SingletonClassNode => {
                chain.push(Ancestor::Scope);
            }
            _ => chain.push(Ancestor::Other),
        }
    }
    let mut ancestors = chain.into_iter();
    let mut ancestor = ancestors.next();
    if ancestor == Some(Ancestor::Begin) {
        ancestor = ancestors.next();
    }
    ancestor.is_none() || ancestor == Some(Ancestor::Scope)
}

/// The terminator of a heredoc (`loc.heredoc_end`), without its newline.
fn heredoc_end(ctx: &Context<'_>, node: &Node<'_>) -> Option<Span> {
    let closing = match node.kind() {
        NodeKind::StringNode => node.as_string_node()?.closing_loc(),
        NodeKind::InterpolatedStringNode => node.as_interpolated_string_node()?.closing_loc(),
        NodeKind::XStringNode => node.as_x_string_node().map(|n| n.closing_loc()),
        NodeKind::InterpolatedXStringNode => {
            node.as_interpolated_x_string_node().map(|n| n.closing_loc())
        }
        _ => None,
    }?;
    let mut span = closing.span();
    let text = ctx.text(span);
    let trimmed = text.strip_suffix(b"\n").unwrap_or(text);
    let trimmed = trimmed.strip_suffix(b"\r").unwrap_or(trimmed);
    span.end = span.start + u32::try_from(trimmed.len()).expect("offset exceeds u32");
    Some(span)
}

/// `TLetCorrection#replace_t_let`: replaces `T.let(value, Type)` with `value`,
/// keeping the bodies of heredocs that open in the value but end after it.
fn replace_t_let(ctx: &Context<'_>, t_let_node: &Node<'_>, value_node: &Node<'_>) -> Edit {
    let t_let_span = t_let_node.span();
    let value_span = value_node.span();

    // `tail_heredocs`
    let mut heredocs: Vec<(Span, Span)> = Vec::new();
    let mut collect = |node: &Node<'_>| {
        if is_heredoc(node) {
            if let Some(end) = heredoc_end(ctx, node) {
                if end.end > value_span.end {
                    heredocs.push((node.span(), end));
                }
            }
        }
    };
    collect(value_node);
    each_descendant(value_node, &mut collect);

    if heredocs.is_empty() {
        return Edit::replace(t_let_span, ctx.text(value_span).to_vec());
    }

    let source = ctx.source().bytes();
    // `range_including_heredocs`
    let end = heredocs.iter().map(|(_, end)| end.end).fold(t_let_span.end, u32::max);

    // `source_including_heredocs`
    heredocs.sort_by_key(|(opening, _)| opening.start);
    let mut text = ctx.text(value_span).to_vec();
    text.extend_from_slice(&marker_line_comment(source, value_span.end as usize));

    // Bodies follow the line holding the markers, one after another.
    let mut cursor = value_span.end as usize;
    let mut body_start = source[cursor..]
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(source.len(), |offset| cursor + offset + 1);
    for (_, terminator) in &heredocs {
        text.push(b'\n');
        text.extend_from_slice(&source[body_start..terminator.end as usize]);
        cursor = terminator.end as usize;
        body_start = source[cursor..]
            .iter()
            .position(|&byte| byte == b'\n')
            .map_or(source.len(), |offset| cursor + offset + 1);
    }
    Edit::replace(Span::new(t_let_span.start, end), text)
}

/// The comment trailing the heredoc marker, with its leading space.
fn marker_line_comment(source: &[u8], value_end: usize) -> Vec<u8> {
    let line_end = source[value_end..]
        .iter()
        .position(|&byte| byte == b'\n')
        .map_or(source.len(), |offset| value_end + offset);
    let rest = &source[value_end..line_end];
    match rest.iter().position(|&byte| byte == b'#') {
        Some(index) => {
            let mut comment = vec![b' '];
            comment.extend_from_slice(&rest[index..]);
            comment
        }
        None => Vec::new(),
    }
}
