//! rubocop-rails' `ActiveRecordHelper` mixin
//! (`lib/rubocop/cop/mixin/active_record_helper.rb`), for the parts that
//! work from a model class: its table name and its `belongs_to` lookups.

use linter::{Context, NodeInfo};
use ruby_ast::node::ClassNode;
use ruby_ast::{each_descendant, for_each_child, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

use crate::inflector::tableize;
use crate::schema::Table;

/// One `belongs_to :name, ...` call in a model class.
#[derive(Debug, Clone)]
pub struct BelongsTo {
    /// The association name (first argument).
    pub name: String,
    /// `foreign_key:` when given as a symbol or string.
    pub foreign_key: Option<String>,
    /// `polymorphic: true`.
    pub polymorphic: bool,
}

/// What the helper needs to know about one model class.
#[derive(Debug, Clone)]
pub struct ClassInfo {
    /// The class's source span.
    pub span: Span,
    /// `ActiveRecordHelper#table_name`.
    pub table_name: String,
    /// `find_belongs_to`, in source order.
    pub belongs_to: Vec<BelongsTo>,
}

/// The innermost class enclosing the node being entered (`class_node`:
/// `node.each_ancestor.find(&:class_type?)`).
#[must_use]
pub fn enclosing_class(ctx: &Context<'_>) -> Option<NodeInfo> {
    ctx.ancestors().iter().rev().find(|info| info.kind == NodeKind::ClassNode).copied()
}

/// Locates the class at `class_span` under `ctx`'s root and describes it.
#[must_use]
pub fn class_info(ctx: &Context<'_>, class_span: Span) -> Option<ClassInfo> {
    let root = ctx.parsed().root();
    let mut namespaces = Vec::new();
    let class = locate(&root, class_span, &mut namespaces)?;

    let mut table_name = None;
    let mut belongs_to = Vec::new();
    each_descendant(&class.as_node(), &mut |node| {
        let Some(call) = node.as_call_node() else { return };
        if call.is_safe_navigation() {
            return;
        }
        let args: Vec<Node<'_>> =
            call.arguments().map(|a| a.arguments().iter().collect()).unwrap_or_default();
        match call.name().as_slice() {
            // `(send self :table_name= {str sym})`; the last match wins.
            b"table_name=" => {
                if call.receiver().is_some_and(|r| r.as_self_node().is_some()) && args.len() == 1 {
                    if let Some(name) = text(&args[0]) {
                        table_name = Some(name);
                    }
                }
            }
            // `(send nil? :belongs_to {str sym} ...)`.
            b"belongs_to" => {
                if call.receiver().is_none() {
                    if let Some(name) = args.first().and_then(text) {
                        belongs_to.push(belongs_to_options(name, &args));
                    }
                }
            }
            _ => {}
        }
    });

    let table_name = table_name.unwrap_or_else(|| {
        // `[*class_nodes, *namespaces].reverse.map { |n| n.children[1] }.join('_').tableize`
        let mut names = defined_module_names(&class.constant_path());
        names.extend(namespaces.iter().rev().cloned());
        names.reverse();
        tableize(&names.join("_"))
    });
    Some(ClassInfo { span: class_span, table_name, belongs_to })
}

impl ClassInfo {
    /// `resolve_relation_into_column`: `name` itself when the table has such
    /// a column, else the foreign key (plus `<name>_type` when polymorphic) of
    /// the `belongs_to` called `name`; `None` when it cannot be resolved.
    #[must_use]
    pub fn resolve_relation_into_column(
        &self,
        name: &str,
        table: Option<&Table>,
    ) -> Option<Vec<String>> {
        let table = table?;
        if table.with_column(name) {
            return Some(vec![name.to_string()]);
        }
        for belongs_to in &self.belongs_to {
            if belongs_to.name != name {
                continue;
            }
            let foreign_key =
                belongs_to.foreign_key.clone().unwrap_or_else(|| format!("{name}_id"));
            if !table.with_column(&foreign_key) {
                continue;
            }
            return Some(if belongs_to.polymorphic {
                vec![foreign_key, format!("{name}_type")]
            } else {
                vec![foreign_key]
            });
        }
        None
    }
}

fn belongs_to_options(name: String, args: &[Node<'_>]) -> BelongsTo {
    let mut foreign_key = None;
    let mut polymorphic = false;
    if let Some(last) = args.last() {
        let elements = if let Some(hash) = last.as_keyword_hash_node() {
            Some(hash.elements())
        } else {
            last.as_hash_node().map(|hash| hash.elements())
        };
        for pair in elements.iter().flat_map(|e| e.iter()).filter_map(|e| e.as_assoc_node()) {
            let Some(key) = pair.key().as_symbol_node().map(|k| k.unescaped().to_vec()) else {
                continue;
            };
            let value = pair.value();
            if key == b"foreign_key" && foreign_key.is_none() {
                foreign_key = text(&value);
            } else if key == b"polymorphic" && value.as_true_node().is_some() {
                polymorphic = true;
            }
        }
    }
    BelongsTo { name, foreign_key, polymorphic }
}

/// The value of a `str` or `sym` node.
fn text(node: &Node<'_>) -> Option<String> {
    if let Some(symbol) = node.as_symbol_node() {
        return Some(String::from_utf8_lossy(symbol.unescaped()).into_owned());
    }
    node.as_string_node().map(|string| String::from_utf8_lossy(string.unescaped()).into_owned())
}

/// The last name of a class or module's constant path (`identifier.children[1]`).
fn last_name(path: &Node<'_>) -> Option<String> {
    if let Some(read) = path.as_constant_read_node() {
        return Some(String::from_utf8_lossy(read.name().as_slice()).into_owned());
    }
    let path = path.as_constant_path_node()?;
    Some(String::from_utf8_lossy(path.name()?.as_slice()).into_owned())
}

/// `defined_module.each_node.map { |node| node.children[1] }`: the const
/// itself, then its scopes outward; a leading `::` contributes an empty name.
fn defined_module_names(path: &Node<'_>) -> Vec<String> {
    let mut names = Vec::new();
    let mut node = path.as_constant_path_node();
    if node.is_none() {
        names.extend(last_name(path));
        return names;
    }
    while let Some(path_node) = node {
        names.extend(path_node.name().map(|n| String::from_utf8_lossy(n.as_slice()).into_owned()));
        match path_node.parent() {
            Some(parent) => {
                if let Some(next) = parent.as_constant_path_node() {
                    node = Some(next);
                } else {
                    names.extend(last_name(&parent));
                    node = None;
                }
            }
            None => {
                names.push(String::new());
                node = None;
            }
        }
    }
    names
}

/// Finds the `ClassNode` at `target`, collecting the names of the classes
/// and modules enclosing it, outermost first.
fn locate<'a>(
    node: &Node<'a>,
    target: Span,
    namespaces: &mut Vec<String>,
) -> Option<ClassNode<'a>> {
    if node.span() == target {
        if let Some(class) = node.as_class_node() {
            return Some(class);
        }
    }
    let namespace = node
        .as_class_node()
        .map(|c| c.constant_path())
        .or_else(|| node.as_module_node().map(|m| m.constant_path()))
        .and_then(|path| last_name(&path));
    let pushed = namespace.is_some();
    if let Some(name) = namespace {
        namespaces.push(name);
    }
    let mut found = None;
    for_each_child(node, |child| {
        let span = child.span();
        if found.is_none() && span.start <= target.start && target.end <= span.end {
            found = locate(child, target, namespaces);
        }
    });
    if found.is_none() && pushed {
        namespaces.pop();
    }
    found
}
