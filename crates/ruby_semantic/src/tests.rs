//! Ported assertions from RuboCop's `spec/rubocop/cop/variable_force_spec.rb`
//! and `spec/rubocop/cop/variable_force/{assignment,reference,scope,variable,
//! variable_table}_spec.rb`.
//!
//! The upstream specs poke `VariableTable` directly with synthesised
//! `s(:def)` nodes; there is no equivalent here (the table only exists while
//! the builder runs), so each one is restated as the Ruby snippet that
//! produces the same table and checked through the finished [`Semantics`].

use ruby_ast::{NodeExt as _, Parsed};
use ruby_source::SourceFile;

use super::*;

/// Parses `text` and runs `check` against its model.
fn semantics(text: &str, check: impl FnOnce(&Semantics<'_>, &SourceFile)) {
    let source = SourceFile::new("test.rb", text.as_bytes().to_vec());
    let parsed = Parsed::parse(&source);
    assert!(!parsed.has_errors(), "snippet does not parse: {text}");
    let model = Semantics::build(&parsed.root());
    check(&model, &source);
}

fn names(model: &Semantics<'_>, scope: ScopeId) -> Vec<String> {
    model
        .scope(scope)
        .variables()
        .iter()
        .map(|&id| String::from_utf8_lossy(model.variable(id).name()).into_owned())
        .collect()
}

fn find(model: &Semantics<'_>, name: &str) -> VariableId {
    model
        .variable_ids()
        .find(|&id| model.variable(id).name() == name.as_bytes())
        .unwrap_or_else(|| panic!("no variable named {name}"))
}

fn text(source: &SourceFile, span: Span) -> &str {
    std::str::from_utf8(source.slice(span)).expect("utf-8")
}

// --------------------------------------------------------------- scopes

#[test]
fn top_level_code_lives_in_a_naked_top_level_scope() {
    semantics("foo = 1\n", |model, _| {
        assert_eq!(model.scopes().len(), 1);
        assert_eq!(model.scope(model.top_level()).kind(), ScopeKind::TopLevel);
        assert_eq!(names(model, model.top_level()), ["foo"]);
    });
}

/// `Scope#body_node`, for each scope type the upstream spec enumerates.
#[test]
fn body_node_is_the_scope_body_for_every_scope_type() {
    let cases = [
        ("def some_method\n  this_is_target\nend\n", ScopeKind::Def),
        ("def self.some_method\n  this_is_target\nend\n", ScopeKind::Def),
        ("module SomeModule\n  this_is_target\nend\n", ScopeKind::Module),
        ("class SomeClass\n  this_is_target\nend\n", ScopeKind::Class),
        ("class << self\n  this_is_target\nend\n", ScopeKind::SingletonClass),
        ("1.times do\n  this_is_target\nend\n", ScopeKind::Block),
        ("this_is_target\n", ScopeKind::TopLevel),
    ];
    for (snippet, kind) in cases {
        semantics(snippet, |model, source| {
            let scope = model
                .scopes()
                .iter()
                .find(|scope| scope.kind() == kind)
                .unwrap_or_else(|| panic!("no {kind:?} scope in {snippet}"));
            let body = scope.body().expect("a body");
            let statements = body.as_statements_node().expect("statement list");
            let only = statements.body().iter().next().expect("one statement");
            assert_eq!(text(source, only.span()), "this_is_target", "{snippet}");
            assert_eq!(
                text(source, scope.return_value_node().expect("value").span()),
                "this_is_target"
            );
        });
    }
}

/// `Scope#each_node`, outer-boundary half: a class's superclass and a
/// singleton class's expression belong to the enclosing scope, so the bare
/// calls in them are collected there.
#[test]
fn twisted_children_are_scanned_in_the_enclosing_scope() {
    semantics("class SomeClass < outer_call\n  inner_call\nend\n", |model, _| {
        let top = model.scope(model.top_level());
        assert_eq!(top.bare_call_names(), [&b"outer_call"[..]]);
        let class = model.scopes().iter().find(|s| s.kind() == ScopeKind::Class).expect("class");
        assert_eq!(class.bare_call_names(), [&b"inner_call"[..]]);
    });
}

/// `Scope#each_node`, inner-boundary half: a block's own send stays outside,
/// the block body does not.
#[test]
fn block_bodies_are_scanned_in_the_block_scope() {
    semantics("outer_call { inner_call }\n", |model, _| {
        let top = model.scope(model.top_level());
        assert_eq!(top.bare_call_names(), [&b"outer_call"[..]]);
        let block = model.scopes().iter().find(|s| s.kind() == ScopeKind::Block).expect("block");
        assert_eq!(block.bare_call_names(), [&b"inner_call"[..]]);
    });
}

#[test]
fn scopes_are_left_innermost_first() {
    semantics("def outer\n  1.times { inner }\nend\n", |model, _| {
        let kinds: Vec<ScopeKind> =
            model.leave_order().iter().map(|&id| model.scope(id).kind()).collect();
        assert_eq!(kinds, [ScopeKind::Block, ScopeKind::Def, ScopeKind::TopLevel]);
    });
}

// ------------------------------------------------------- variable table

/// `VariableTable#find_variable`: a block sees the enclosing scope, a `def`
/// does not.
#[test]
fn only_block_scopes_reach_outer_variables() {
    semantics("foo = 1\n1.times { foo = 2 }\n", |model, _| {
        assert_eq!(model.variables().len(), 1, "the block assigns the outer variable");
        assert!(model.variable(find(model, "foo")).captured_by_block());
    });
    semantics("foo = 1\ndef m\n  foo = 2\nend\n", |model, _| {
        assert_eq!(model.variables().len(), 2, "the def declares its own variable");
        assert!(model.variables().iter().all(|v| !v.captured_by_block()));
    });
}

#[test]
fn a_block_reaches_through_another_block() {
    semantics("baz = 1\n1.times { 2.times { baz } }\n", |model, _| {
        assert_eq!(model.variables().len(), 1);
        assert!(model.variable(find(model, "baz")).referenced());
    });
}

/// `VariableTable#declare_variable`: re-declaring a name in one scope keeps
/// its slot but resolves to the newer variable.
#[test]
fn redeclaring_a_name_replaces_it_in_its_scope() {
    semantics("1.times { |_foo, bar, _foo| }\n", |model, _| {
        let block =
            model.scopes().iter().position(|s| s.kind() == ScopeKind::Block).expect("block scope");
        let block = ScopeId(u32::try_from(block).expect("small"));
        assert_eq!(names(model, block), ["_foo", "bar"]);
        assert_eq!(model.variables().len(), 3, "all three declarations are recorded");
    });
}

/// `VariableTable#accessible_variables`, exercised through the implicit
/// references a zero-arity `super` makes to every method argument.
#[test]
fn zero_arity_super_references_only_method_arguments() {
    semantics("def m(foo)\n  bar = 1\n  super\nend\n", |model, _| {
        let foo = model.variable(find(model, "foo"));
        assert!(foo.referenced());
        assert!(!foo.references()[0].explicit(), "an implicit read");
        assert!(!model.variable(find(model, "bar")).referenced());
    });
}

#[test]
fn binding_references_every_accessible_variable() {
    semantics("def m\n  foo = 1\n  do_something(binding)\nend\n", |model, _| {
        assert!(model.variable(find(model, "foo")).referenced());
    });
}

// ---------------------------------------------------------- assignments

#[test]
fn an_unassigned_variable_is_only_referenced_by_reads() {
    semantics("def m(foo)\nend\n", |model, _| {
        assert!(!model.variable(find(model, "foo")).referenced());
    });
    semantics("def m(foo)\n  foo\nend\n", |model, _| {
        assert!(model.variable(find(model, "foo")).referenced());
    });
    semantics("def m\n  foo = 1\nend\n", |model, _| {
        assert!(!model.variable(find(model, "foo")).referenced());
    });
    semantics("def m\n  foo = 1\n  foo\nend\n", |model, _| {
        assert!(model.variable(find(model, "foo")).referenced());
    });
}

/// `Assignment#meta_assignment_node` and `#operator`, case by case.
#[test]
fn meta_assignment_and_operator_follow_the_enclosing_construct() {
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Shape {
        Plain,
        Operator,
        Multiple,
    }
    let cases = [
        ("def m\n  foo = 1\nend\n", Shape::Plain, "="),
        ("def m\n  foo += 1\nend\n", Shape::Operator, "+="),
        ("def m\n  foo ||= 1\nend\n", Shape::Operator, "||="),
        ("def m\n  foo &&= 1\nend\n", Shape::Operator, "&&="),
        ("def m\n  foo, bar = [1, 2]\nend\n", Shape::Multiple, "="),
        ("def m\n  *foo = [1, 2]\nend\n", Shape::Multiple, "="),
    ];
    for (snippet, shape, operator) in cases {
        semantics(snippet, |model, source| {
            let id = model.variable(find(model, "foo")).assignments()[0];
            let assignment = model.assignment(id);
            let actual = match assignment.meta() {
                None => Shape::Plain,
                Some(Meta::Operator) => Shape::Operator,
                Some(Meta::Multiple(_)) => Shape::Multiple,
                Some(other) => panic!("unexpected meta {other:?} for {snippet}"),
            };
            assert_eq!(actual, shape, "{snippet}");
            assert_eq!(
                text(source, assignment.operator_span().expect("an operator")),
                operator,
                "{snippet}"
            );
        });
    }
}

#[test]
fn a_for_index_is_a_for_assignment() {
    semantics("def m\n  for item in items\n  end\nend\n", |model, _| {
        let id = model.variable(find(model, "item")).assignments()[0];
        assert!(model.assignment(id).is_for_assignment());
        assert!(model.assignment(id).operator_span().is_none());
    });
}

/// The upstream "when it is an argument of a send node" contexts: an
/// assignment nested inside a call argument has no meta node.
#[test]
fn an_assignment_inside_a_call_argument_has_no_meta_node() {
    let snippets = [
        "def m\n  foo += my_method(bar = 1)\nend\n",
        "def m\n  foo ||= my_method(bar = 1)\nend\n",
        "def m\n  foo, baz = my_method(bar = 1)\nend\n",
        "def m\n  *foo = my_method(bar = 1)\nend\n",
        "def m\n  for item in my_method(bar = 1)\n  end\nend\n",
    ];
    for snippet in snippets {
        semantics(snippet, |model, _| {
            let id = model.variable(find(model, "bar")).assignments()[0];
            assert!(model.assignment(id).meta().is_none(), "{snippet}");
        });
    }
    semantics("def m\n  foo, bar = my_method((baz, quux = [1, 2]))\nend\n", |model, _| {
        let id = model.variable(find(model, "baz")).assignments()[0];
        assert!(model.assignment(id).is_multiple_assignment());
    });
}

#[test]
fn an_exception_variable_is_flagged() {
    semantics("begin\n  x\nrescue Foo => error\nend\n", |model, _| {
        let id = model.variable(find(model, "error")).assignments()[0];
        assert!(model.assignment(id).is_exception());
    });
}

#[test]
fn a_named_capture_declares_and_assigns_every_group() {
    semantics("/(?<foo>a)(?<bar>b)/ =~ 'ab'\n", |model, source| {
        for name in ["foo", "bar"] {
            let variable = find(model, name);
            assert_eq!(model.variable(variable).decl_kind(), DeclKind::RegexpNamedCapture);
            let id = model.variable(variable).assignments()[0];
            assert!(model.assignment(id).is_regexp_named_capture());
            assert_eq!(text(source, model.assignment(id).name_span()), "/(?<foo>a)(?<bar>b)/");
        }
    });
}

/// `VariableForce#process_node` must survive regexps the spec calls out:
/// empty, with a `regopt`, and with a newline right after a group opener.
#[test]
fn unusual_regexp_matches_do_not_panic() {
    for snippet in ["// =~ \"\"\n", "/\\x82/n =~ \"a\"\n", "/(\n pattern\n)/ =~ string\n"] {
        semantics(snippet, |model, _| assert!(model.variables().is_empty(), "{snippet}"));
    }
}

/// A pattern-match variable is declared but never assigned
/// (`process_pattern_match_variable`).
#[test]
fn pattern_variables_declare_without_assigning() {
    semantics("foo in { bar: bar }\n", |model, _| {
        let variable = find(model, "bar");
        assert_eq!(model.variable(variable).decl_kind(), DeclKind::PatternMatch);
        assert!(model.variable(variable).assignments().is_empty());
    });
}

// --------------------------------------------------------- reassignment

#[test]
fn an_unreferenced_assignment_is_reassigned_by_the_next_one() {
    semantics("def m\n  foo = 1\n  foo = 2\n  foo\nend\n", |model, _| {
        let assignments = model.variable(find(model, "foo")).assignments().to_vec();
        assert_eq!(assignments.len(), 2);
        assert!(model.assignment(assignments[0]).reassigned());
        assert!(!model.assignment(assignments[0]).referenced());
        assert!(model.assignment(assignments[1]).referenced());
    });
}

#[test]
fn assignments_in_different_branches_do_not_reassign_each_other() {
    semantics(
        "def m(flag)\n  if flag\n    foo = 1\n  else\n    foo = 2\n  end\n  foo\nend\n",
        |model, _| {
            let assignments = model.variable(find(model, "foo")).assignments().to_vec();
            assert_eq!(assignments.len(), 2);
            assert!(assignments.iter().all(|&id| !model.assignment(id).reassigned()));
            assert!(assignments.iter().all(|&id| model.assignment(id).referenced()));
        },
    );
}

#[test]
fn a_reference_does_not_reach_past_an_exclusive_branch() {
    // The `rescue` clause's assignment is exclusive with the reference in
    // the `else` clause, so only the first assignment is referenced.
    semantics(
        "begin\n  do_something\nrescue\n  foo = 1\nelse\n  foo = 2\n  foo\nend\n",
        |model, _| {
            let assignments = model.variable(find(model, "foo")).assignments().to_vec();
            assert!(!model.assignment(assignments[0]).referenced(), "rescue clause");
            assert!(model.assignment(assignments[1]).referenced(), "else clause");
        },
    );
}

#[test]
fn a_modifier_conditional_assignment_does_not_stop_the_search() {
    semantics("a = nil\nputs a if (a = 123)\n", |model, _| {
        let assignments = model.variable(find(model, "a")).assignments().to_vec();
        assert_eq!(assignments.len(), 2);
        assert!(assignments.iter().all(|&id| model.assignment(id).referenced()));
    });
}

// ---------------------------------------------------------------- loops

#[test]
fn a_loop_marks_its_last_assignment_as_referenced() {
    semantics("def m(param)\n  ret = 1\n  while param != 10\n    param += 2\n    ret = param + 1\n  end\n  ret\nend\n", |model, _| {
        let param = model.variable(find(model, "param"));
        assert!(param.assignments().iter().all(|&id| model.assignment(id).referenced()));
    });
}

#[test]
fn a_post_condition_loop_scans_its_body_first() {
    semantics("begin\n  foo = 1\nend while foo > 10\nputs foo\n", |model, _| {
        let assignments = model.variable(find(model, "foo")).assignments().to_vec();
        assert!(model.assignment(assignments[0]).referenced());
    });
}

#[test]
fn a_rescue_with_retry_behaves_like_a_loop() {
    semantics(
        "retried = false\n\nbegin\n  do_something\nrescue\n  fail if retried\n  retried = true\n  retry\nend\n",
        |model, _| {
            let assignments = model.variable(find(model, "retried")).assignments().to_vec();
            assert_eq!(assignments.len(), 2);
            // The second assignment is only read by the next iteration, which
            // the AST order cannot show.
            assert!(model.assignment(assignments[1]).referenced());
        },
    );
}

// -------------------------------------------------------------- shadows

#[test]
fn a_block_parameter_records_the_variable_it_hides() {
    semantics("def m\n  foo = 1\n  puts foo\n  1.times do |foo|\n  end\nend\n", |model, _| {
        let inner = model
            .variable_ids()
            .filter(|&id| model.variable(id).name() == b"foo")
            .last()
            .expect("two `foo`s");
        let outer = model.variable(inner).shadows().expect("shadowed");
        assert_eq!(model.variable(outer).decl_kind(), DeclKind::Assignment);
        assert_ne!(model.variable(outer).scope(), model.variable(inner).scope());
    });
}

#[test]
fn a_method_parameter_does_not_see_the_class_body() {
    semantics("class C\n  foo = 1\n  puts foo\n  def m(foo)\n  end\nend\n", |model, _| {
        let inner = model
            .variable_ids()
            .filter(|&id| model.variable(id).name() == b"foo")
            .last()
            .expect("two `foo`s");
        assert!(model.variable(inner).shadows().is_none());
    });
}

// ------------------------------------------------------------- branches

#[test]
fn if_branches_are_mutually_exclusive() {
    semantics(
        "def m(flag)\n  if flag\n    foo = 1\n  else\n    foo = 2\n  end\nend\n",
        |model, _| {
            let assignments = model.variable(find(model, "foo")).assignments().to_vec();
            let first = model.assignment(assignments[0]).branch().expect("then branch");
            let second = model.assignment(assignments[1]).branch().expect("else branch");
            assert_ne!(first, second);
            assert!(model.exclusive_with(first, second));
            assert!(model.exclusive_with(second, first));
        },
    );
}

#[test]
fn the_main_body_of_a_rescue_may_jump_to_another_branch() {
    semantics("begin\n  foo = 1\nrescue\n  bar = 2\nend\n", |model, _| {
        let main = model
            .assignment(model.variable(find(model, "foo")).assignments()[0])
            .branch()
            .expect("main body branch");
        let clause = model
            .assignment(model.variable(find(model, "bar")).assignments()[0])
            .branch()
            .expect("rescue clause branch");
        assert_eq!(model.branch(main).kind(), BranchKind::Rescue);
        assert!(model.branch(main).may_jump_to_other_branch());
        assert!(model.branch(main).may_run_incompletely());
        assert!(!model.exclusive_with(main, clause), "the main body may jump into the clause");
        assert!(model.exclusive_with(clause, main));
    });
}

#[test]
fn an_ensure_body_wraps_the_rescue_branches() {
    semantics("begin\n  foo = 1\nrescue\n  foo = 2\nensure\n  bar = 3\nend\n", |model, _| {
        let main = model
            .assignment(model.variable(find(model, "foo")).assignments()[0])
            .branch()
            .expect("main body branch");
        let parent = model.branch(main).parent().expect("an enclosing ensure branch");
        assert_eq!(model.branch(parent).kind(), BranchKind::Ensure);
        // The ensure body always runs, so it is not itself a branch.
        assert!(model
            .assignment(model.variable(find(model, "bar")).assignments()[0])
            .branch()
            .is_none());
    });
}

#[test]
fn an_always_run_alternative_is_not_a_branch() {
    // `for` evaluates its index and collection unconditionally.
    semantics("for item in items\n  x = 1\nend\n", |model, _| {
        assert!(model
            .assignment(model.variable(find(model, "item")).assignments()[0])
            .branch()
            .is_none());
        let body = model
            .assignment(model.variable(find(model, "x")).assignments()[0])
            .branch()
            .expect("loop body branch");
        assert_eq!(model.branch(body).kind(), BranchKind::For);
    });
}
