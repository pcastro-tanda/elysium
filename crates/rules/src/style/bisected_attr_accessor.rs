//! `Style/BisectedAttrAccessor`, ported from RuboCop's
//! `lib/rubocop/cop/style/bisected_attr_accessor.rb` plus the private
//! `Macro` helper it requires from
//! `lib/rubocop/cop/style/bisected_attr_accessor/macro.rb`.

use linter::{
    Applicability, Context, Department, Edit, Fix, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::node::CallNode;
use ruby_ast::{Node, NodeExt as _, NodeKind};
use ruby_source::Span;

/// Checks for places where `attr_reader` and `attr_writer` for the same
/// method can be combined into single `attr_accessor`.
#[derive(Debug, Clone)]
pub struct BisectedAttrAccessor;

impl Rule for BisectedAttrAccessor {
    const META: RuleMeta = RuleMeta {
        name: "Style/BisectedAttrAccessor",
        department: Department::Style,
        summary: "Checks for places where `attr_reader` and `attr_writer` for the same method \
                  can be combined into single `attr_accessor`.",
        explanation: "\
```ruby
# bad
class Foo
  attr_reader :bar
  attr_writer :bar
end

# good
class Foo
  attr_accessor :bar
end
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::Safe,
        stability: Stability::Stable,
        kinds: &[NodeKind::ClassNode, NodeKind::ModuleNode, NodeKind::SingletonClassNode],
        config: &[],
        blind_spots: "",
    };

    fn configure(_options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self)
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        let body = match node {
            Node::ClassNode { .. } => node.as_class_node().expect("kind matched").body(),
            Node::ModuleNode { .. } => node.as_module_node().expect("kind matched").body(),
            Node::SingletonClassNode { .. } => {
                node.as_singleton_class_node().expect("kind matched").body()
            }
            _ => return,
        };
        let Some(stmts_node) = body.as_ref().and_then(Node::as_statements_node) else { return };
        let stmts: Vec<Node<'_>> = stmts_node.body().iter().collect();

        // RuboCop's `find_macros`: collect every `attr_reader`/`attr_writer`/`attr`
        // call in the class body and its (Ruby-`VisibilityHelp`) visibility.
        let mut macros: Vec<Macro> = Vec::new();
        for (i, stmt) in stmts.iter().enumerate() {
            let Some(call) = stmt.as_call_node() else { continue };
            let Some(kind) = macro_kind(&call) else { continue };
            macros.push(Macro {
                span: stmt.span(),
                attrs: collect_attrs(ctx, &call),
                kind,
                visibility: node_visibility(&stmts, i),
                bisection: Vec::new(),
            });
        }

        // RuboCop's `group_by(&:visibility)`, in first-seen order.
        let mut groups: Vec<(&'static [u8], Vec<usize>)> = Vec::new();
        for (idx, m) in macros.iter().enumerate() {
            match groups.iter_mut().find(|(v, _)| *v == m.visibility) {
                Some((_, idxs)) => idxs.push(idx),
                None => groups.push((m.visibility, vec![idx])),
            }
        }

        let mut to_rewrite: Vec<usize> = Vec::new();

        for (_, idxs) in &groups {
            // RuboCop's `find_bisection`: intersection of reader attr names and
            // writer attr names, in reader-encounter order, deduplicated.
            let mut reader_names: Vec<String> = Vec::new();
            let mut writer_names: Vec<String> = Vec::new();
            for &i in idxs {
                let m = &macros[i];
                let names = m.attrs.iter().map(|(n, _)| n.clone());
                match m.kind {
                    MacroKind::Writer => writer_names.extend(names),
                    MacroKind::Reader => reader_names.extend(names),
                }
            }
            let mut bisected: Vec<String> = Vec::new();
            for n in &reader_names {
                if writer_names.contains(n) && !bisected.contains(n) {
                    bisected.push(n.clone());
                }
            }
            if bisected.is_empty() {
                continue;
            }

            for &i in idxs {
                // `Macro#bisect`: this macro's own attrs restricted to (and
                // reordered by) the group's bisected names.
                let bisection: Vec<(String, Span)> = bisected
                    .iter()
                    .filter_map(|name| {
                        macros[i]
                            .attrs
                            .iter()
                            .find(|(n, _)| n == name)
                            .map(|(n, span)| (n.clone(), *span))
                    })
                    .collect();
                if bisection.is_empty() {
                    continue;
                }
                macros[i].bisection = bisection;
                to_rewrite.push(i);
            }
        }

        for i in to_rewrite {
            report_and_correct(ctx, &macros[i]);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MacroKind {
    Reader,
    Writer,
}

/// RuboCop's `Macro`, restricted to what `BisectedAttrAccessor` needs.
/// Holds only byte offsets and owned text, so it borrows nothing from the
/// parse tree.
struct Macro {
    span: Span,
    attrs: Vec<(String, Span)>,
    kind: MacroKind,
    visibility: &'static [u8],
    bisection: Vec<(String, Span)>,
}

/// RuboCop's `Macro.macro?` plus `#reader?`/`#writer?`: a receiver-less call
/// to `attr_reader`, `attr_writer` or `attr`.
fn macro_kind(call: &CallNode<'_>) -> Option<MacroKind> {
    if call.receiver().is_some() {
        return None;
    }
    match call.name().as_slice() {
        b"attr_reader" | b"attr" => Some(MacroKind::Reader),
        b"attr_writer" => Some(MacroKind::Writer),
        _ => None,
    }
}

/// RuboCop's `Macro#initialize`: `node.arguments.to_h { |attr| [attr.source, attr] }`.
/// A later argument sharing an earlier one's source text overwrites its span
/// but keeps the first-seen position, matching `Hash#to_h`'s dedup rules.
fn collect_attrs(ctx: &Context<'_>, call: &CallNode<'_>) -> Vec<(String, Span)> {
    let mut out: Vec<(String, Span)> = Vec::new();
    let Some(args) = call.arguments() else { return out };
    for arg in &args.arguments() {
        let span = arg.span();
        let text = String::from_utf8_lossy(ctx.text(span)).into_owned();
        if let Some(existing) = out.iter_mut().find(|(n, _)| *n == text) {
            existing.1 = span;
        } else {
            out.push((text, span));
        }
    }
    out
}

/// RuboCop's `VisibilityHelp::VISIBILITY_SCOPES` bare-call matcher (`(send
/// nil? {:private :protected :public})`).
fn is_visibility_block(call: &CallNode<'_>) -> bool {
    call.receiver().is_none()
        && call.arguments().is_none_or(|a| a.arguments().is_empty())
        && matches!(call.name().as_slice(), b"public" | b"protected" | b"private")
}

/// RuboCop's `VisibilityHelp#node_visibility`, restricted to the
/// `node_visibility_from_visibility_block` path: accessor macros are never
/// `def`s, so the `node_visibility_from_visibility_inline` path never
/// matches here.
fn node_visibility(stmts: &[Node<'_>], i: usize) -> &'static [u8] {
    for stmt in stmts[..i].iter().rev() {
        if let Some(call) = stmt.as_call_node() {
            if is_visibility_block(&call) {
                return match call.name().as_slice() {
                    b"private" => b"private",
                    b"protected" => b"protected",
                    _ => b"public",
                };
            }
        }
    }
    b"public"
}

/// RuboCop's `register_offense` plus `after_class`'s correction: every
/// bisected attr on this macro gets its own offense, all sharing the one
/// correction the whole macro node needs. The fix engine applies the first
/// of these identical fixes and skips the rest as overlapping, which is
/// harmless since they touch exactly the same bytes with exactly the same
/// replacement.
fn report_and_correct(ctx: &mut Context<'_>, m: &Macro) {
    let range = ctx.whole_lines(m.span);
    let indent = " ".repeat(ctx.line_col(m.span.start).column as usize);

    let bisected_names: Vec<&str> = m.bisection.iter().map(|(n, _)| n.as_str()).collect();
    let rest: Vec<&str> =
        m.attrs.iter().map(|(n, _)| n.as_str()).filter(|n| !bisected_names.contains(n)).collect();
    let all_bisected = rest.is_empty();

    let edits = match m.kind {
        MacroKind::Writer => {
            if all_bisected {
                vec![Edit::delete(range)]
            } else {
                vec![Edit::replace(m.span, format!("attr_writer {}", rest.join(", ")).into_bytes())]
            }
        }
        MacroKind::Reader => {
            let attr_accessor = format!("attr_accessor {}\n", bisected_names.join(", "));
            if all_bisected {
                vec![Edit::replace(range, format!("{indent}{attr_accessor}").into_bytes())]
            } else {
                vec![
                    Edit::insert(m.span.start, attr_accessor.into_bytes()),
                    Edit::replace(
                        m.span,
                        format!("{indent}attr_reader {}", rest.join(", ")).into_bytes(),
                    ),
                ]
            }
        }
    };

    for (name, span) in &m.bisection {
        ctx.report_with_fix(
            &BisectedAttrAccessor::META,
            *span,
            format!("Combine both accessors into `attr_accessor {name}`."),
            Fix { applicability: Applicability::Safe, edits: edits.clone() },
        );
    }
}
