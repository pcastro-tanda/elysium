//! `Style/HashSyntax`, ported from RuboCop's `lib/rubocop/cop/style/hash_syntax.rb`
//! plus the `HashShorthandSyntax` mixin
//! (`lib/rubocop/cop/mixin/hash_shorthand_syntax.rb`) it includes.
//!
//! Prism gives every hash literal an [`NodeKind::HashNode`] (braced, e.g.
//! `{a: 1}`) or [`NodeKind::KeywordHashNode`] (braceless bare keyword
//! arguments, e.g. `foo a: 1`) -- this maps directly onto RuboCop's own
//! `braces?` distinction, both dispatch through the same `on_hash`-style
//! entry point here. Pair nodes ([`NodeKind::AssocNode`]) are read directly
//! off `elements()`; `**splat` entries ([`NodeKind::AssocSplatNode`]) are
//! skipped, matching RuboCop's `node.pairs`.
//!
//! The mixin's parenthesization/ancestor logic (`require_hash_value?`,
//! `def_node_that_require_parentheses`, `last_expression?`, ...) needs
//! parent/sibling information Prism's value-typed nodes don't carry
//! natively. `file_start` walks the whole tree once with a generic
//! [`ruby_ast::Visitor`] to build a parent map, an ordered sibling-list map,
//! and a small set of precomputed per-node facts (parenthesized?,
//! modifier-form?, assignment-chain?, ...); the per-hash checks below query
//! those maps instead of re-deriving them from raw nodes.

use std::collections::HashMap;

use linter::{
    Applicability, ConfigDefault, ConfigOption, Context, Department, Edit, Fix, FixAvailability,
    OptionError, Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use ruby_ast::node::AssocNode;
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind, Visitor};
use ruby_source::Span;

const MSG_19: &str = "Use the new Ruby 1.9 hash syntax.";
const MSG_NO_MIXED_KEYS: &str = "Don't mix styles in the same hash.";
const MSG_HASH_ROCKETS: &str = "Use hash rockets syntax.";
const OMIT_HASH_VALUE_MSG: &str = "Omit the hash value.";
const EXPLICIT_HASH_VALUE_MSG: &str = "Include the hash value.";
const DO_NOT_MIX_OMIT_VALUE_MSG: &str =
    "Do not mix explicit and implicit hash values. Omit the hash value.";
const DO_NOT_MIX_EXPLICIT_VALUE_MSG: &str =
    "Do not mix explicit and implicit hash values. Include the hash value.";

/// RuboCop's `EnforcedStyle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    Ruby19,
    HashRockets,
    NoMixedKeys,
    Ruby19NoMixedKeys,
}

/// RuboCop's `EnforcedShorthandSyntax`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Shorthand {
    Either,
    Always,
    Never,
    Consistent,
    EitherConsistent,
}

/// How a pair's value relates to `EnforcedShorthandSyntax: consistent`/
/// `either_consistent`'s per-hash breakdown (RuboCop's
/// `breakdown_value_types_of_hash`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Bucket {
    Omitted,
    Needed,
    Omittable,
}

/// A node identity: span plus kind, unique enough to key the ancestor maps
/// (two distinct nodes sharing both a span and a kind do not occur in
/// practice for the node kinds these maps track).
type Key = (Span, NodeKind);

/// Precomputed facts about one node, gathered while it is visited (before
/// its children), since only live Prism node handles expose them.
#[derive(Debug, Clone, Default)]
#[allow(clippy::struct_excessive_bools)]
struct Facts {
    /// `Call`/`Super`/`Yield`: has explicit parentheses around its arguments.
    parenthesized: bool,
    /// `Call`: is `[]` or `[]=` (RuboCop's `brackets?`).
    is_bracket: bool,
    /// `Call`: `assignment_method?` (a `foo=`-shaped writer, not a comparison).
    is_assignment_method: bool,
    /// `Call`: the receiver's span, if any.
    receiver: Option<Span>,
    /// `Call`: `message_loc`. `Super`/`Yield`: `keyword_loc`.
    selector: Option<Span>,
    /// `Call`/`Super`/`Yield`: argument spans, in source order.
    args: Vec<Span>,
    /// `If`/`Unless`/`While`/`Until`: written in modifier (no `end`) form.
    is_modifier: bool,
    /// One of the plain-assignment node kinds RuboCop-AST's `assignment?`
    /// recognizes (`lvasgn`/`ivasgn`/`cvasgn`/`gvasgn`/`casgn`/`masgn`/
    /// `op_asgn`/`or_asgn`/`and_asgn`); attribute/index writers (`send`)
    /// are deliberately excluded, matching RuboCop-AST.
    is_chain_assignment: bool,
    /// A literal `( ... )` grouping.
    is_parens_group: bool,
}

/// Parent, sibling-order, and fact lookups built once per file by
/// [`MapBuilder`].
#[derive(Debug, Default, Clone)]
struct AncestorMaps {
    parent_of: HashMap<Key, Key>,
    children_of: HashMap<Key, Vec<Key>>,
    facts: HashMap<Key, Facts>,
}

impl AncestorMaps {
    fn facts_of(&self, key: Key) -> Facts {
        self.facts.get(&key).cloned().unwrap_or_default()
    }

    fn has_right_sibling(&self, key: Key) -> bool {
        let Some(parent) = self.parent_of.get(&key) else { return false };
        let Some(kids) = self.children_of.get(parent) else { return false };
        match kids.iter().position(|k| *k == key) {
            Some(i) => i + 1 < kids.len(),
            None => false,
        }
    }

    /// RuboCop's `find_ancestor_method_dispatch_node`: the nearest `Call`/
    /// `Super`/`Yield` ancestor of the hash at `hash_key` (transparently
    /// skipping the [`NodeKind::ArgumentsNode`] wrapper Prism inserts,
    /// which whitequark's flatter AST has no equivalent of), excluding
    /// `[]`/`[]=` calls.
    fn dispatch_ancestor(&self, hash_key: Key) -> Option<Key> {
        let parent = *self.parent_of.get(&hash_key)?;
        let ancestor = if parent.1 == NodeKind::ArgumentsNode {
            *self.parent_of.get(&parent)?
        } else {
            parent
        };
        if !matches!(ancestor.1, NodeKind::CallNode | NodeKind::SuperNode | NodeKind::YieldNode) {
            return None;
        }
        if self.facts_of(ancestor).is_bracket {
            return None;
        }
        Some(ancestor)
    }

    /// RuboCop's `method_dispatch_as_argument?`: `key`'s effective parent
    /// (skipping the `ArgumentsNode` wrapper) is itself a `Call`/`Super`/
    /// `Yield` node.
    fn method_dispatch_as_argument(&self, key: Key) -> bool {
        let Some(&parent) = self.parent_of.get(&key) else { return false };
        let effective = if parent.1 == NodeKind::ArgumentsNode {
            self.parent_of.get(&parent).copied()
        } else {
            Some(parent)
        };
        matches!(
            effective.map(|e| e.1),
            Some(NodeKind::CallNode | NodeKind::SuperNode | NodeKind::YieldNode)
        )
    }

    /// RuboCop's `last_expression?`.
    fn last_expression(&self, key: Key) -> bool {
        if self.has_right_sibling(key) {
            return false;
        }
        let mut cur = self.parent_of.get(&key).copied();
        let mut assignment = None;
        while let Some(k) = cur {
            if self.facts_of(k).is_chain_assignment {
                assignment = Some(k);
                break;
            }
            cur = self.parent_of.get(&k).copied();
        }
        let Some(mut asg) = assignment else { return true };
        while let Some(&p) = self.parent_of.get(&asg) {
            if self.facts_of(p).is_chain_assignment {
                asg = p;
            } else {
                break;
            }
        }
        !self.has_right_sibling(asg)
    }

    /// RuboCop's `require_hash_value_for_around_hash_literal?`.
    fn require_value_for_around_literal(&self, hash_key: Key) -> bool {
        let Some(ancestor) = self.dispatch_ancestor(hash_key) else { return false };
        if hash_key.1 == NodeKind::HashNode {
            // `!node.parent.braces?`: a braced hash is never the receiver
            // of, or a bare keyword-argument to, anything that would need
            // disambiguating.
            return false;
        }
        let facts = self.facts_of(ancestor);
        if facts.receiver == Some(hash_key.0) {
            // `use_element_of_hash_literal_as_receiver?`.
            return false;
        }
        if facts.parenthesized {
            return false;
        }
        let mut cur = self.parent_of.get(&ancestor).copied();
        while let Some(k) = cur {
            if self.facts_of(k).is_modifier {
                return true;
            }
            cur = self.parent_of.get(&k).copied();
        }
        false
    }

    /// RuboCop's `def_node_that_require_parentheses`, returning the
    /// whitespace span to replace with `(` and the offset to insert `)`
    /// after, if the call needs wrapping.
    fn parens_needed(&self, hash_key: Key, last_pair_eligible: bool) -> Option<(Span, u32)> {
        if !last_pair_eligible {
            return None;
        }
        let ancestor = self.dispatch_ancestor(hash_key)?;
        let facts = self.facts_of(ancestor);
        if facts.is_assignment_method || facts.parenthesized {
            return None;
        }
        if let Some(&parent) = self.parent_of.get(&ancestor) {
            if self.facts_of(parent).is_parens_group {
                return None;
            }
        }
        if self.last_expression(ancestor) && !self.method_dispatch_as_argument(ancestor) {
            return None;
        }
        if facts.args.is_empty() {
            return None;
        }
        let first_arg = *facts.args.first()?;
        let last_arg = *facts.args.last()?;
        let selector = facts.selector?;
        Some((Span::new(selector.end, first_arg.start), last_arg.end))
    }
}

/// Walks the whole tree once (independent of [`HashSyntax`]'s own
/// `enter`/`leave` subscription) to build [`AncestorMaps`].
struct MapBuilder {
    stack: Vec<Key>,
    maps: AncestorMaps,
}

impl<'pr> Visitor<'pr> for MapBuilder {
    fn enter(&mut self, node: &Node<'pr>) {
        let key: Key = (node.span(), node.kind());
        if let Some(&parent) = self.stack.last() {
            self.maps.parent_of.insert(key, parent);
            self.maps.children_of.entry(parent).or_default().push(key);
        }
        self.stack.push(key);
        if let Some(facts) = compute_facts(node) {
            self.maps.facts.insert(key, facts);
        }
    }

    fn leave(&mut self, _node: &Node<'pr>) {
        self.stack.pop();
    }
}

/// `foo=`-shaped writer that is not a comparison method (RuboCop-AST's
/// `assignment_method?`).
fn is_assignment_method_name(name: &[u8]) -> bool {
    name.ends_with(b"=") && !matches!(name, b"==" | b"!=" | b"<=" | b">=" | b"<=>" | b"===")
}

fn compute_facts(node: &Node<'_>) -> Option<Facts> {
    match node.kind() {
        NodeKind::CallNode => {
            let call = node.as_call_node().expect("kind matched");
            let opening = call.opening_loc().map(|l| l.as_slice());
            let is_bracket = opening.is_some_and(|s| s.starts_with(b"["));
            let parenthesized = opening.is_some_and(|s| s.starts_with(b"("));
            let name = call.message_loc().map_or(b"".as_slice(), |l| l.as_slice());
            let args = call
                .arguments()
                .map(|a| a.arguments().iter().map(|n| n.span()).collect())
                .unwrap_or_default();
            Some(Facts {
                parenthesized,
                is_bracket,
                is_assignment_method: !is_bracket
                    && (call.is_attribute_write() || is_assignment_method_name(name)),
                receiver: call.receiver().map(|r| r.span()),
                selector: call.message_loc().map(|l| l.span()).or(Some(call.location().span())),
                args,
                ..Facts::default()
            })
        }
        NodeKind::SuperNode => {
            let sup = node.as_super_node().expect("kind matched");
            let args = sup
                .arguments()
                .map(|a| a.arguments().iter().map(|n| n.span()).collect())
                .unwrap_or_default();
            Some(Facts {
                parenthesized: sup.lparen_loc().is_some(),
                selector: Some(sup.keyword_loc().span()),
                args,
                ..Facts::default()
            })
        }
        NodeKind::YieldNode => {
            let y = node.as_yield_node().expect("kind matched");
            let args = y
                .arguments()
                .map(|a| a.arguments().iter().map(|n| n.span()).collect())
                .unwrap_or_default();
            Some(Facts {
                parenthesized: y.lparen_loc().is_some(),
                selector: Some(y.keyword_loc().span()),
                args,
                ..Facts::default()
            })
        }
        NodeKind::IfNode => {
            let n = node.as_if_node().expect("kind matched");
            Some(Facts {
                is_modifier: n.if_keyword_loc().is_some() && n.end_keyword_loc().is_none(),
                ..Facts::default()
            })
        }
        NodeKind::UnlessNode => {
            let n = node.as_unless_node().expect("kind matched");
            Some(Facts { is_modifier: n.end_keyword_loc().is_none(), ..Facts::default() })
        }
        NodeKind::WhileNode => {
            let n = node.as_while_node().expect("kind matched");
            Some(Facts { is_modifier: n.closing_loc().is_none(), ..Facts::default() })
        }
        NodeKind::UntilNode => {
            let n = node.as_until_node().expect("kind matched");
            Some(Facts { is_modifier: n.closing_loc().is_none(), ..Facts::default() })
        }
        NodeKind::ParenthesesNode => Some(Facts { is_parens_group: true, ..Facts::default() }),
        NodeKind::LocalVariableWriteNode
        | NodeKind::LocalVariableAndWriteNode
        | NodeKind::LocalVariableOrWriteNode
        | NodeKind::LocalVariableOperatorWriteNode
        | NodeKind::InstanceVariableWriteNode
        | NodeKind::InstanceVariableAndWriteNode
        | NodeKind::InstanceVariableOrWriteNode
        | NodeKind::InstanceVariableOperatorWriteNode
        | NodeKind::ClassVariableWriteNode
        | NodeKind::ClassVariableAndWriteNode
        | NodeKind::ClassVariableOrWriteNode
        | NodeKind::ClassVariableOperatorWriteNode
        | NodeKind::GlobalVariableWriteNode
        | NodeKind::GlobalVariableAndWriteNode
        | NodeKind::GlobalVariableOrWriteNode
        | NodeKind::GlobalVariableOperatorWriteNode
        | NodeKind::ConstantWriteNode
        | NodeKind::ConstantAndWriteNode
        | NodeKind::ConstantOrWriteNode
        | NodeKind::ConstantOperatorWriteNode
        | NodeKind::ConstantPathWriteNode
        | NodeKind::ConstantPathAndWriteNode
        | NodeKind::ConstantPathOrWriteNode
        | NodeKind::ConstantPathOperatorWriteNode
        | NodeKind::MultiWriteNode => Some(Facts { is_chain_assignment: true, ..Facts::default() }),
        _ => None,
    }
}

/// The text of `key`'s symbol/label with its colon (leading for `:sym =>`,
/// trailing for `sym:`) stripped, matching RuboCop-AST's key/value source
/// on top of whitequark (Prism keeps the colon as part of the label's own
/// span; whitequark does not).
fn stripped_symbol_text(key_text: &[u8], has_rocket: bool) -> &[u8] {
    if has_rocket {
        key_text.strip_prefix(b":").unwrap_or(key_text)
    } else {
        key_text.strip_suffix(b":").unwrap_or(key_text)
    }
}

/// Ruby's `/\A[_a-z]\w*[?!]?\z/i`.
fn is_plain_identifier(sym: &[u8]) -> bool {
    let Some((&first, rest)) = sym.split_first() else { return false };
    if !(first == b'_' || first.is_ascii_alphabetic()) {
        return false;
    }
    let body = match rest.last() {
        Some(b'?' | b'!') => &rest[..rest.len() - 1],
        _ => rest,
    };
    body.iter().all(|&b| b == b'_' || b.is_ascii_alphanumeric())
}

/// Looks for hash literal syntax matching the configured style, and for
/// Ruby 3.1 hash-value-shorthand consistency.
#[derive(Debug, Clone)]
pub struct HashSyntax {
    style: Style,
    shorthand: Shorthand,
    use_hash_rockets_with_symbol_values: bool,
    prefer_hash_rockets_for_non_alnum_ending_symbols: bool,
    target_ruby_version: f64,
    maps: AncestorMaps,
}

impl HashSyntax {
    fn word_symbol_pair(&self, pair: &AssocNode<'_>) -> bool {
        let key = pair.key();
        // RuboCop-AST's `any_sym_type?` covers both plain (`:sym`/`sym:`)
        // and interpolated (`"#{x}":`) symbols.
        if key.as_symbol_node().is_none() && key.as_interpolated_symbol_node().is_none() {
            return false;
        }
        let key_text = key.location().as_slice();
        let sym = stripped_symbol_text(key_text, pair.operator_loc().is_some());
        self.acceptable_19_syntax_symbol(sym)
    }

    fn acceptable_19_syntax_symbol(&self, sym: &[u8]) -> bool {
        if self.prefer_hash_rockets_for_non_alnum_ending_symbols {
            let ends_alnum_or_quote = match sym.last() {
                Some(&b) => b.is_ascii_alphanumeric() || b == b'"' || b == b'\'' || b >= 0x80,
                None => false,
            };
            if !ends_alnum_or_quote {
                return false;
            }
        }
        if is_plain_identifier(sym) {
            return true;
        }
        if self.target_ruby_version <= 2.1 {
            return false;
        }
        (sym.len() >= 2 && sym.starts_with(b"'") && sym.ends_with(b"'"))
            || (sym.len() >= 2 && sym.starts_with(b"\"") && sym.ends_with(b"\""))
    }

    fn sym_indices(&self, pairs: &[AssocNode<'_>]) -> bool {
        pairs.iter().all(|p| self.word_symbol_pair(p))
    }

    fn force_hash_rockets(&self, pairs: &[AssocNode<'_>]) -> bool {
        self.use_hash_rockets_with_symbol_values
            && pairs.iter().any(|p| p.value().as_symbol_node().is_some())
    }

    /// RuboCop's `check`: flags every pair whose current delimiter matches
    /// `flag_colon` (colon/label form when `true`, hash-rocket form when
    /// `false`), with a fix chosen per RuboCop's own `autocorrect`.
    #[allow(clippy::too_many_arguments)]
    fn check(
        &self,
        hash: &Node<'_>,
        hash_key: Key,
        pairs: &[AssocNode<'_>],
        flag_colon: bool,
        msg: &'static str,
        force_rockets: bool,
        ctx: &mut Context<'_>,
    ) {
        let mut wrapped_in_braces = false;
        for pair in pairs {
            let is_colon = pair.operator_loc().is_none();
            if is_colon != flag_colon {
                continue;
            }
            let key_span = pair.key().span();
            let op_span = operator_span(pair, key_span);
            let offense_span = Span::new(pair.location().span().start, op_span.end);
            let to_rockets = self.style == Style::HashRockets
                || force_rockets
                || (matches!(self.style, Style::NoMixedKeys | Style::Ruby19NoMixedKeys)
                    && is_colon);
            let fix = if to_rockets {
                fix_to_hash_rockets(pair)
            } else {
                let wrap = !wrapped_in_braces && needs_brace_wrap(&self.maps, hash_key);
                if wrap {
                    wrapped_in_braces = true;
                }
                fix_to_ruby19(&self.maps, hash_key, pair, hash, wrap)
            };
            ctx.report_with_fix(&Self::META, offense_span, msg, fix);
        }
    }

    fn check_style(
        &self,
        hash: &Node<'_>,
        hash_key: Key,
        pairs: &[AssocNode<'_>],
        ctx: &mut Context<'_>,
    ) {
        let force_rockets = self.force_hash_rockets(pairs);
        if self.style == Style::HashRockets || force_rockets {
            self.check(hash, hash_key, pairs, true, MSG_HASH_ROCKETS, force_rockets, ctx);
            return;
        }
        match self.style {
            Style::Ruby19NoMixedKeys => {
                if self.sym_indices(pairs) {
                    self.check(hash, hash_key, pairs, false, MSG_19, false, ctx);
                } else {
                    self.check(hash, hash_key, pairs, true, MSG_NO_MIXED_KEYS, false, ctx);
                }
            }
            Style::NoMixedKeys => {
                if self.sym_indices(pairs) {
                    let first_is_colon = pairs.first().is_some_and(|p| p.operator_loc().is_none());
                    self.check(
                        hash,
                        hash_key,
                        pairs,
                        !first_is_colon,
                        MSG_NO_MIXED_KEYS,
                        false,
                        ctx,
                    );
                } else {
                    self.check(hash, hash_key, pairs, true, MSG_NO_MIXED_KEYS, false, ctx);
                }
            }
            Style::Ruby19 => {
                if self.sym_indices(pairs) {
                    self.check(hash, hash_key, pairs, false, MSG_19, false, ctx);
                }
            }
            Style::HashRockets => unreachable!("handled above"),
        }
    }

    /// RuboCop's `require_hash_value?`.
    fn require_hash_value(&self, hash_key: Key, pair: &AssocNode<'_>) -> bool {
        if pair.key().as_symbol_node().is_none() {
            return true;
        }
        if self.maps.require_value_for_around_literal(hash_key) {
            return true;
        }
        let value = pair.value();
        if value.as_call_node().is_none() && value.as_local_variable_read_node().is_none() {
            return true;
        }
        let key_text = stripped_symbol_text(pair.key().location().as_slice(), false);
        let value_text = value.location().as_slice();
        key_text != value_text || key_text.ends_with(b"!") || key_text.ends_with(b"?")
    }

    /// RuboCop's `register_offense` for the shorthand mixin: replaces the
    /// whole pair, optionally also wrapping the enclosing bare call in
    /// parentheses when omitting the last pair's value would otherwise be
    /// ambiguous.
    #[allow(clippy::too_many_arguments)]
    fn emit_shorthand(
        &self,
        hash_key: Key,
        pair: &AssocNode<'_>,
        last_pair: &AssocNode<'_>,
        msg: &'static str,
        replacement: Vec<u8>,
        offense_span: Span,
        adds_parens: bool,
        ctx: &mut Context<'_>,
    ) {
        let mut edits = vec![Edit::replace(pair.location().span(), replacement)];
        if adds_parens {
            let last_eligible = pair_is_shortenable(last_pair);
            if let Some((open, close_at)) = self.maps.parens_needed(hash_key, last_eligible) {
                if pair.location().span() == last_pair.location().span() {
                    edits.push(Edit::replace(open, b"(".to_vec()));
                    edits.push(Edit::insert(close_at, b")".to_vec()));
                } else {
                    // The opening paren only needs inserting once; attach it
                    // to every offending pair is harmless in RuboCop (same
                    // edit, deduped by its rewriter) but our fixes apply
                    // independently, so only the pair that is itself the
                    // last one carries both edits, and earlier pairs in the
                    // same hash carry neither (the last pair's own fix, or
                    // a not-yet-processed sibling, supplies them).
                }
            }
        }
        let fix = Fix { applicability: Applicability::Safe, edits };
        ctx.report_with_fix(&Self::META, offense_span, msg, fix);
    }

    fn check_pair_shorthand(
        &self,
        hash_key: Key,
        pair: &AssocNode<'_>,
        last_pair: &AssocNode<'_>,
        ctx: &mut Context<'_>,
    ) {
        let omitted = pair.value().as_implicit_node().is_some();
        match self.shorthand {
            Shorthand::Always => {
                if omitted || self.require_hash_value(hash_key, pair) {
                    return;
                }
                self.emit_omit(hash_key, pair, last_pair, OMIT_HASH_VALUE_MSG, ctx);
            }
            Shorthand::Never => {
                if !omitted {
                    return;
                }
                emit_include(pair, EXPLICIT_HASH_VALUE_MSG, ctx);
            }
            Shorthand::Either | Shorthand::Consistent | Shorthand::EitherConsistent => {}
        }
    }

    fn emit_omit(
        &self,
        hash_key: Key,
        pair: &AssocNode<'_>,
        last_pair: &AssocNode<'_>,
        msg: &'static str,
        ctx: &mut Context<'_>,
    ) {
        let replacement = pair.key().location().as_slice().to_vec();
        let offense_span = pair.value().span();
        self.emit_shorthand(hash_key, pair, last_pair, msg, replacement, offense_span, true, ctx);
    }
}

fn emit_include(pair: &AssocNode<'_>, msg: &'static str, ctx: &mut Context<'_>) {
    let key_text = stripped_symbol_text(pair.key().location().as_slice(), false);
    let mut replacement = key_text.to_vec();
    replacement.extend_from_slice(b": ");
    replacement.extend_from_slice(key_text);
    let implicit_span = pair.value().span();
    let offense_span = Span::new(implicit_span.start, implicit_span.end.saturating_sub(1));
    let edits = vec![Edit::replace(pair.location().span(), replacement)];
    let fix = Fix { applicability: Applicability::Safe, edits };
    ctx.report_with_fix(&HashSyntax::META, offense_span, msg, fix);
}

impl HashSyntax {
    fn check_mixed_shorthand(&self, hash_key: Key, pairs: &[AssocNode<'_>], ctx: &mut Context<'_>) {
        if self.target_ruby_version <= 3.0
            || (hash_key.1 != NodeKind::HashNode && hash_key.1 != NodeKind::KeywordHashNode)
        {
            return;
        }
        let Some(last_pair) = pairs.last() else { return };
        let buckets: Vec<(Bucket, &AssocNode<'_>)> = pairs
            .iter()
            .map(|p| {
                let b = if p.value().as_implicit_node().is_some() {
                    Bucket::Omitted
                } else if self.require_hash_value(hash_key, p) {
                    Bucket::Needed
                } else {
                    Bucket::Omittable
                };
                (b, p)
            })
            .collect();
        let has_omitted = buckets.iter().any(|(b, _)| *b == Bucket::Omitted);
        let has_needed = buckets.iter().any(|(b, _)| *b == Bucket::Needed);
        let has_omittable = buckets.iter().any(|(b, _)| *b == Bucket::Omittable);
        let mixed =
            usize::from(has_omitted) + usize::from(has_needed) + usize::from(has_omittable) > 1;
        if mixed {
            if has_needed {
                for (b, p) in &buckets {
                    if *b == Bucket::Omitted {
                        emit_include(p, DO_NOT_MIX_EXPLICIT_VALUE_MSG, ctx);
                    }
                }
            } else {
                for (b, p) in &buckets {
                    if *b == Bucket::Omittable {
                        self.emit_omit(hash_key, p, last_pair, DO_NOT_MIX_OMIT_VALUE_MSG, ctx);
                    }
                }
            }
            return;
        }
        if has_needed {
            return;
        }
        if has_omittable && self.shorthand == Shorthand::EitherConsistent {
            return;
        }
        if has_omittable {
            for (_, p) in &buckets {
                self.emit_omit(hash_key, p, last_pair, OMIT_HASH_VALUE_MSG, ctx);
            }
        }
    }

    fn check_shorthand(&self, hash_key: Key, pairs: &[AssocNode<'_>], ctx: &mut Context<'_>) {
        if self.target_ruby_version <= 3.0 {
            return;
        }
        match self.shorthand {
            Shorthand::Either => {}
            Shorthand::Always | Shorthand::Never => {
                let Some(last_pair) = pairs.last() else { return };
                for pair in pairs {
                    self.check_pair_shorthand(hash_key, pair, last_pair, ctx);
                }
            }
            Shorthand::Consistent | Shorthand::EitherConsistent => {
                self.check_mixed_shorthand(hash_key, pairs, ctx);
            }
        }
    }
}

/// A pair whose value could textually be omitted (its own last-pair
/// eligibility for the paren-wrapping check): either already omitted, or
/// an explicit value whose text equals the (colon-stripped) key text.
fn pair_is_shortenable(pair: &AssocNode<'_>) -> bool {
    if pair.value().as_implicit_node().is_some() {
        return true;
    }
    let key_text = stripped_symbol_text(pair.key().location().as_slice(), false);
    key_text == pair.value().location().as_slice()
}

/// The operator's span: `AssocNode::operator_loc()` when present (hash
/// rocket), else the trailing colon byte of the (label-form) key, which
/// Prism includes in the key's own span.
fn operator_span(pair: &AssocNode<'_>, key_span: Span) -> Span {
    match pair.operator_loc() {
        Some(loc) => loc.span(),
        None => Span::new(key_span.end.saturating_sub(1), key_span.end),
    }
}

/// RuboCop's `argument_without_space?`: the hash starts exactly where an
/// enclosing bare call's selector ends (no space between them).
fn argument_without_space(maps: &AncestorMaps, hash_key: Key) -> bool {
    let Some(ancestor) = maps.dispatch_ancestor(hash_key) else { return false };
    let Some(selector) = maps.facts_of(ancestor).selector else { return false };
    selector.end == hash_key.0.start
}

/// RuboCop's `hash_node.parent&.return_type? && !hash_node.braces?`.
fn needs_brace_wrap(maps: &AncestorMaps, hash_key: Key) -> bool {
    if hash_key.1 != NodeKind::KeywordHashNode {
        return false;
    }
    let Some(&parent) = maps.parent_of.get(&hash_key) else { return false };
    let effective = if parent.1 == NodeKind::ArgumentsNode {
        maps.parent_of.get(&parent).copied()
    } else {
        Some(parent)
    };
    effective.is_some_and(|e| e.1 == NodeKind::ReturnNode)
}

fn fix_to_ruby19(
    maps: &AncestorMaps,
    hash_key: Key,
    pair: &AssocNode<'_>,
    hash: &Node<'_>,
    wrap_in_braces: bool,
) -> Fix {
    let key_text = pair.key().location().as_slice();
    let sym = stripped_symbol_text(key_text, true);
    let pair_span = pair.location().span();
    let value_span = pair.value().span();
    let mut replacement = Vec::new();
    if argument_without_space(maps, hash_key) {
        replacement.push(b' ');
    }
    replacement.extend_from_slice(sym);
    replacement.extend_from_slice(b": ");
    let prefix = Span::new(pair_span.start, value_span.start);
    let mut edits = vec![Edit::replace(prefix, replacement)];
    if wrap_in_braces {
        let hash_span = hash.span();
        edits.push(Edit::insert(hash_span.start, b"{".to_vec()));
        edits.push(Edit::insert(hash_span.end, b"}".to_vec()));
    }
    Fix { applicability: Applicability::Safe, edits }
}

fn fix_to_hash_rockets(pair: &AssocNode<'_>) -> Fix {
    let key_text = pair.key().location().as_slice();
    let has_rocket = pair.operator_loc().is_some();
    let sym = stripped_symbol_text(key_text, has_rocket);
    let is_omitted = pair.value().as_implicit_node().is_some();
    let mut replacement = Vec::with_capacity(sym.len() * 2 + 5);
    replacement.push(b':');
    replacement.extend_from_slice(sym);
    replacement.extend_from_slice(b" => ");
    if is_omitted {
        replacement.extend_from_slice(sym);
    }
    let pair_span = pair.location().span();
    // A value-omission pair's `value()` is an `ImplicitNode` whose span
    // mirrors the key's own span (Prism has no separate value text to
    // preserve there), so the whole pair must be replaced; otherwise use
    // the prefix up to the value's own start, preserving its source text.
    let range =
        if is_omitted { pair_span } else { Span::new(pair_span.start, pair.value().span().start) };
    Fix { applicability: Applicability::Safe, edits: vec![Edit::replace(range, replacement)] }
}

impl Rule for HashSyntax {
    const META: RuleMeta = RuleMeta {
        name: "Style/HashSyntax",
        department: Department::Style,
        summary:
            "Prefer Ruby 1.9 hash syntax `{ a: 1, b: 2 }` over 1.8 syntax `{ :a => 1, :b => 2 }`.",
        explanation: "\
Checks hash literal syntax.

It can enforce either the use of the classic hash rocket syntax or the use
of the newer Ruby 1.9 syntax (when applicable).

A separate offense is registered for each problematic pair.

* `ruby19` (default) - forces use of the 1.9 syntax (e.g. `{a: 1}`) when
  hashes have all symbols for keys.
* `hash_rockets` - forces use of hash rockets for all hashes.
* `no_mixed_keys` - simply checks for hashes with mixed syntaxes.
* `ruby19_no_mixed_keys` - forces use of ruby 1.9 syntax and forbids mixed
  syntax hashes.

```ruby
# EnforcedStyle: ruby19 (default)
# bad
{:a => 2}
{b: 1, :c => 2}

# good
{a: 2, b: 1}
{:c => 2, 'd' => 2} # acceptable since 'd' isn't a symbol
{d: 1, 'e' => 2} # technically not forbidden
```

This cop also has an `EnforcedShorthandSyntax` option for Ruby 3.1's hash
value omission syntax (default `either`):

* `always` - forces use of the 3.1 syntax (e.g. `{foo:}`).
* `never` - forces use of explicit hash literal value.
* `either` - accepts both shorthand and explicit use of hash literal value.
* `consistent` - forces use of the 3.1 syntax only if all values can be
  omitted in the hash.
* `either_consistent` - accepts both shorthand and explicit use of hash
  literal value, but they must be consistent within a hash.",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Nursery,
        kinds: &[NodeKind::HashNode, NodeKind::KeywordHashNode],
        config: &[
            ConfigOption {
                name: "EnforcedStyle",
                default: ConfigDefault::Str("ruby19"),
                allowed: &["ruby19", "hash_rockets", "no_mixed_keys", "ruby19_no_mixed_keys"],
                doc: "Which hash key syntax to enforce.",
            },
            ConfigOption {
                name: "EnforcedShorthandSyntax",
                default: ConfigDefault::Str("either"),
                allowed: &["always", "never", "either", "consistent", "either_consistent"],
                doc: "Whether to enforce Ruby 3.1's hash value shorthand (`{foo:}`).",
            },
            ConfigOption {
                name: "UseHashRocketsWithSymbolValues",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Force hashes that have a symbol value to use hash rockets.",
            },
            ConfigOption {
                name: "PreferHashRocketsForNonAlnumEndingSymbols",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Do not suggest `{ a: 1 }` over `{ :a? => 1 }` in ruby19 style.",
            },
        ],
        blind_spots: "\
`acceptable_19_syntax_symbol?`'s identifier check uses ASCII
`[A-Za-z_]\\w*[?!]?`, not Ruby's Unicode-aware `\\w`: a non-ASCII symbol name
is treated as unacceptable for ruby19 conversion (kept as a hash rocket),
which is the safe (false-negative) direction, not RuboCop's exact behavior.
Likewise `PreferHashRocketsForNonAlnumEndingSymbols`'s `\\p{Alnum}` check
treats any non-ASCII byte as alnum rather than checking the actual Unicode
category.

The `EnforcedShorthandSyntax` mixin's parenthesization logic
(`def_node_that_require_parentheses`, `last_expression?`,
`method_dispatch_as_argument?`) is re-derived here from a generic
parent/sibling map built once per file (see the module docs), rather than
RuboCop-AST's live `node.parent`/`node.right_sibling`/`each_ancestor`; it
matches RuboCop's cop-spec-verified cases but has not been proven against
every possible nesting.

`TargetRubyVersion` is read via `AllCops` peer options for the `<= 3.0`
(shorthand syntax unsupported) and `<= 2.1` (quoted-symbol ruby19 syntax
unsupported) gates; when absent, both default to a modern Ruby (shorthand
enabled, quoted symbols allowed), matching RuboCop's own
`DEFAULT_RUBY_VERSION`.

When a hash with multiple shorthand-omittable pairs needs its enclosing
bare call wrapped in parentheses, only the pair that is itself the hash's
last pair carries the paren-adding edits (RuboCop's rewriter would merge
identical edits from every offending pair's own correction block; this
engine's fixes are independent, so attaching them to every pair would make
every pair but one's fix collide and get dropped instead).",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let style = match options.style("EnforcedStyle")? {
            "hash_rockets" => Style::HashRockets,
            "no_mixed_keys" => Style::NoMixedKeys,
            "ruby19_no_mixed_keys" => Style::Ruby19NoMixedKeys,
            _ => Style::Ruby19,
        };
        let shorthand = match options.style("EnforcedShorthandSyntax")? {
            "always" => Shorthand::Always,
            "never" => Shorthand::Never,
            "consistent" => Shorthand::Consistent,
            "either_consistent" => Shorthand::EitherConsistent,
            _ => Shorthand::Either,
        };
        let target_ruby_version = options
            .peer("AllCops", "TargetRubyVersion")
            .and_then(linter::OptionValue::as_float)
            .unwrap_or(3.4);
        Ok(Self {
            style,
            shorthand,
            use_hash_rockets_with_symbol_values: options.bool("UseHashRocketsWithSymbolValues"),
            prefer_hash_rockets_for_non_alnum_ending_symbols: options
                .bool("PreferHashRocketsForNonAlnumEndingSymbols"),
            target_ruby_version,
            maps: AncestorMaps::default(),
        })
    }

    fn file_start(&mut self, ctx: &mut Context<'_>) {
        let mut builder = MapBuilder { stack: Vec::new(), maps: AncestorMaps::default() };
        ruby_ast::walk(&ctx.parsed().root(), &mut builder);
        self.maps = builder.maps;
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let elements = match node.kind() {
            NodeKind::HashNode => node.as_hash_node().expect("kind matched").elements(),
            NodeKind::KeywordHashNode => {
                node.as_keyword_hash_node().expect("kind matched").elements()
            }
            _ => return,
        };
        let pairs: Vec<AssocNode<'_>> = elements.iter().filter_map(|n| n.as_assoc_node()).collect();
        if pairs.is_empty() {
            return;
        }
        let hash_key: Key = (node.span(), node.kind());
        self.check_shorthand(hash_key, &pairs, ctx);
        self.check_style(node, hash_key, &pairs, ctx);
    }
}
