//! `Lint/DuplicateHashKey`, ported from RuboCop's
//! `lib/rubocop/cop/lint/duplicate_hash_key.rb` plus the `Duplication` mixin
//! it includes and rubocop-ast's `Node#recursive_basic_literal?`/
//! `Node#basic_literal?` (`lib/rubocop/ast/node.rb`), which the cop relies on
//! to decide which hash keys are even worth comparing.

use linter::{
    Context, Department, FixAvailability, OptionError, Rule, RuleMeta, RuleOptions, Severity,
    Stability,
};
use ruby_ast::node::NodeList;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// RuboCop's `MSG`.
const MSG: &str = "Duplicated key in hash literal.";

/// `Lint::DuplicateHashKey`.
#[derive(Debug, Clone)]
pub struct DuplicateHashKey;

impl Rule for DuplicateHashKey {
    const META: RuleMeta = RuleMeta {
        name: "Lint/DuplicateHashKey",
        department: Department::Lint,
        summary: "Checks for duplicated keys in hash literals.",
        explanation: "\
Checks for duplicated keys in hash literals. This cop considers both
primitive types and constants for the hash keys.

This cop mirrors a warning in Ruby 2.2.

```ruby
# bad
hash = { food: 'apple', food: 'orange' }

# good
hash = { food: 'apple', other_food: 'orange' }
```",
        enabled_by_default: true,
        severity: Severity::Warning,
        fix: FixAvailability::None,
        stability: Stability::Nursery,
        kinds: &[NodeKind::HashNode, NodeKind::KeywordHashNode],
        config: &[],
        blind_spots: "\
Keys are compared by rubocop-ast's `recursive_basic_literal?`/`const_type?`
rules (mirroring `Parser::AST::Node#eql?`'s `[type, children]` structural
equality, not source text), covering `nil`/`true`/`false`, integers,
floats, strings and symbols (plain or interpolated with literal-only
parts), regexps (their `i`/`x`/`m`/`o` flags and, when interpolated,
literal-only parts), arrays and hashes of such literals, `and`/`or` and the
comparison-operator/`!`/`*`/`<=>` sends rubocop-ast treats as recursively
literal, endless/beginless ranges of such literals, parenthesized
single-statement wrappers, and bare or namespaced constant references.
Rational and complex literals, backtick/`%x` command strings, and any
interpolation with zero or more than one embedded statement are never
recognized as comparable keys (rubocop-ast's `recursive_basic_literal?`
would still descend into some of these; false negatives here, never false
positives).",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let elements = match node {
            Node::HashNode { .. } => node.as_hash_node().expect("kind matched").elements(),
            Node::KeywordHashNode { .. } => {
                node.as_keyword_hash_node().expect("kind matched").elements()
            }
            _ => return,
        };
        check_hash(ctx, &elements);
    }
}

/// RuboCop's `on_hash`: gathers the pairs' keys that
/// `recursive_basic_literal?` (or, failing that, `const_type?`) recognizes
/// as comparable, then reports every duplicate after its group's first
/// occurrence (`Duplication#consecutive_duplicates`).
fn check_hash(ctx: &mut Context<'_>, elements: &NodeList<'_>) {
    let mut candidates: Vec<(Key, Span)> = Vec::new();
    for element in elements {
        let Some(assoc) = element.as_assoc_node() else { continue };
        let key_node = assoc.key();
        let Some(value) = key_value(&key_node) else { continue };
        candidates.push((value, report_span(&key_node)));
    }
    if candidates.len() < 2 {
        return;
    }

    let mut groups: Vec<(Key, Vec<Span>)> = Vec::new();
    for (value, span) in candidates {
        match groups.iter_mut().find(|(key, _)| *key == value) {
            Some((_, spans)) => spans.push(span),
            None => groups.push((value, vec![span])),
        }
    }
    for (_, spans) in groups {
        for span in spans.into_iter().skip(1) {
            ctx.report(&DuplicateHashKey::META, span, MSG);
        }
    }
}

/// The span RuboCop actually reports for a hash key: whitequark's parser
/// gives a label-form symbol key (`fruit: 'apple'`) an `sym` node whose own
/// `loc.expression` is just the identifier, with the trailing `:` folded
/// into the pair's separate operator location; Prism instead extends a
/// label `SymbolNode`'s own location through that trailing colon, so its
/// `value_loc` (the identifier alone) is used here instead.
fn report_span(key: &Node<'_>) -> Span {
    match key.as_symbol_node().and_then(|s| s.value_loc()) {
        Some(value_loc) => value_loc.span(),
        None => key.span(),
    }
}

/// `Lint::LITERAL_RECURSIVE_METHODS` (rubocop-ast's `Node`): the only
/// `send`-node method names a key expression may use and still count as a
/// recursive basic literal -- `COMPARISON_OPERATORS + [*, !, <=>]`.
fn is_literal_recursive_method(name: &[u8]) -> bool {
    matches!(name, b"==" | b"===" | b"!=" | b"<=" | b">=" | b">" | b"<" | b"*" | b"!" | b"<=>")
}

/// A structural, value-based fingerprint of one hash key expression,
/// mirroring `Parser::AST::Node#eql?`'s `[type, children]` comparison (not
/// source text): `'a'` and `"a"` compare equal; `1` and `1.0` do not
/// (distinct variants), matching Ruby's `Integer#eql?`/`Float#eql?`.
#[derive(Debug, Clone, PartialEq)]
enum Key {
    Nil,
    True,
    False,
    /// `Integer#to_u32_digits()`: sign plus little-endian digit limbs, exact
    /// for arbitrary-precision integer literals.
    Int(bool, Vec<u32>),
    Float(f64),
    Str(Vec<u8>),
    Sym(Vec<u8>),
    Regexp {
        ignore_case: bool,
        extended: bool,
        multi_line: bool,
        once: bool,
        parts: Vec<Key>,
    },
    DStr(Vec<Key>),
    DSym(Vec<Key>),
    Array(Vec<Key>),
    Hash(Vec<(Key, Key)>),
    And(Box<Key>, Box<Key>),
    Or(Box<Key>, Box<Key>),
    Range(Option<Box<Key>>, Option<Box<Key>>, bool),
    Call(Vec<u8>, Box<Key>, Vec<Key>),
    /// Segments root-first, plus whether the path starts with a leading
    /// `::` (an absolute top-level reference).
    Const(Vec<Vec<u8>>, bool),
}

/// RuboCop's `key.recursive_basic_literal? || key.const_type?`.
fn key_value(node: &Node<'_>) -> Option<Key> {
    recursive_basic_literal(node).or_else(|| const_key(node))
}

/// rubocop-ast's `Node#recursive_basic_literal?`.
fn recursive_basic_literal(node: &Node<'_>) -> Option<Key> {
    match node {
        Node::NilNode { .. } => Some(Key::Nil),
        Node::TrueNode { .. } => Some(Key::True),
        Node::FalseNode { .. } => Some(Key::False),
        Node::IntegerNode { .. } => {
            let n = node.as_integer_node()?;
            let value = n.value();
            let (negative, digits) = value.to_u32_digits();
            Some(Key::Int(negative, digits.to_vec()))
        }
        Node::FloatNode { .. } => Some(Key::Float(node.as_float_node()?.value())),
        Node::StringNode { .. } => Some(Key::Str(node.as_string_node()?.unescaped().to_vec())),
        Node::SymbolNode { .. } => Some(Key::Sym(node.as_symbol_node()?.unescaped().to_vec())),
        Node::RegularExpressionNode { .. } => {
            let n = node.as_regular_expression_node()?;
            Some(Key::Regexp {
                ignore_case: n.is_ignore_case(),
                extended: n.is_extended(),
                multi_line: n.is_multi_line(),
                once: n.is_once(),
                parts: vec![Key::Str(n.unescaped().to_vec())],
            })
        }
        Node::InterpolatedRegularExpressionNode { .. } => {
            let n = node.as_interpolated_regular_expression_node()?;
            Some(Key::Regexp {
                ignore_case: n.is_ignore_case(),
                extended: n.is_extended(),
                multi_line: n.is_multi_line(),
                once: n.is_once(),
                parts: collect_all(&n.parts())?,
            })
        }
        Node::InterpolatedStringNode { .. } => {
            Some(Key::DStr(collect_all(&node.as_interpolated_string_node()?.parts())?))
        }
        Node::InterpolatedSymbolNode { .. } => {
            Some(Key::DSym(collect_all(&node.as_interpolated_symbol_node()?.parts())?))
        }
        Node::ArrayNode { .. } => Some(Key::Array(collect_all(&node.as_array_node()?.elements())?)),
        Node::HashNode { .. } => Some(Key::Hash(collect_pairs(&node.as_hash_node()?.elements())?)),
        Node::KeywordHashNode { .. } => {
            Some(Key::Hash(collect_pairs(&node.as_keyword_hash_node()?.elements())?))
        }
        Node::AndNode { .. } => {
            let n = node.as_and_node()?;
            let left = recursive_basic_literal(&n.left())?;
            let right = recursive_basic_literal(&n.right())?;
            Some(Key::And(Box::new(left), Box::new(right)))
        }
        Node::OrNode { .. } => {
            let n = node.as_or_node()?;
            let left = recursive_basic_literal(&n.left())?;
            let right = recursive_basic_literal(&n.right())?;
            Some(Key::Or(Box::new(left), Box::new(right)))
        }
        Node::RangeNode { .. } => {
            let n = node.as_range_node()?;
            let left = match n.left() {
                Some(side) => Some(Box::new(recursive_basic_literal(&side)?)),
                None => None,
            };
            let right = match n.right() {
                Some(side) => Some(Box::new(recursive_basic_literal(&side)?)),
                None => None,
            };
            Some(Key::Range(left, right, n.is_exclude_end()))
        }
        Node::CallNode { .. } => {
            let n = node.as_call_node()?;
            let name = n.name();
            if !is_literal_recursive_method(name.as_slice()) {
                return None;
            }
            let receiver = recursive_basic_literal(&n.receiver()?)?;
            let args = match n.arguments() {
                Some(args) => collect_all(&args.arguments())?,
                None => Vec::new(),
            };
            Some(Key::Call(name.as_slice().to_vec(), Box::new(receiver), args))
        }
        Node::ParenthesesNode { .. } => {
            let stmts = node.as_parentheses_node()?.body()?.as_statements_node()?;
            recursive_basic_literal(&single_statement(&stmts)?)
        }
        Node::EmbeddedStatementsNode { .. } => {
            let stmts = node.as_embedded_statements_node()?.statements()?;
            recursive_basic_literal(&single_statement(&stmts)?)
        }
        _ => None,
    }
}

/// A `StatementsNode` with exactly one statement, standing in for that
/// statement itself (`ParenthesesNode`/`EmbeddedStatementsNode` bodies with
/// zero or several statements are left unrecognized: a documented blind
/// spot rather than guessed-at semantics).
fn single_statement<'pr>(stmts: &ruby_ast::node::StatementsNode<'pr>) -> Option<Node<'pr>> {
    let body = stmts.body();
    if body.len() == 1 {
        body.first()
    } else {
        None
    }
}

fn collect_all(list: &NodeList<'_>) -> Option<Vec<Key>> {
    list.iter().map(|n| recursive_basic_literal(&n)).collect()
}

/// `HashNode#pairs`/`#keys`: only `AssocNode` (`pair`) elements count; a
/// `**splat` (`AssocSplatNode`) makes the enclosing literal unrecognized.
fn collect_pairs(list: &NodeList<'_>) -> Option<Vec<(Key, Key)>> {
    list.iter()
        .map(|element| {
            let assoc = element.as_assoc_node()?;
            let key = recursive_basic_literal(&assoc.key())?;
            let value = recursive_basic_literal(&assoc.value())?;
            Some((key, value))
        })
        .collect()
}

/// RuboCop's `key.const_type?` half of the filter: a bare or namespaced
/// constant reference, compared by its written path (segments plus whether
/// it starts with a leading `::`).
fn const_key(node: &Node<'_>) -> Option<Key> {
    match node {
        Node::ConstantReadNode { .. } => {
            let name = node.as_constant_read_node()?.name().as_slice().to_vec();
            Some(Key::Const(vec![name], false))
        }
        Node::ConstantPathNode { .. } => {
            let n = node.as_constant_path_node()?;
            let name = n.name()?.as_slice().to_vec();
            match n.parent() {
                None => Some(Key::Const(vec![name], true)),
                Some(parent) => {
                    let Key::Const(mut segments, absolute) = const_key(&parent)? else {
                        return None;
                    };
                    segments.push(name);
                    Some(Key::Const(segments, absolute))
                }
            }
        }
        _ => None,
    }
}
