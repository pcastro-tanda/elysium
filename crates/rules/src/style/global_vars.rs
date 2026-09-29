//! `Style/GlobalVars`, ported from RuboCop's
//! `lib/rubocop/cop/style/global_vars.rb`.
//!
//! Upstream's `on_gvar`/`on_gvasgn` fire for every whitequark `gvar`/`gvasgn`
//! node, which covers plain reads/writes as well as the inner target of
//! `$foo ||= 1` (`or-asgn`), `$foo &&= 1` (`and-asgn`), `$foo += 1`
//! (`op-asgn`), and each element of a multiple assignment's left-hand side.
//! Prism splits these into separate node kinds, so this port subscribes to
//! [`NodeKind::GlobalVariableReadNode`], [`NodeKind::GlobalVariableWriteNode`],
//! [`NodeKind::GlobalVariableOrWriteNode`],
//! [`NodeKind::GlobalVariableAndWriteNode`],
//! [`NodeKind::GlobalVariableOperatorWriteNode`], and
//! [`NodeKind::GlobalVariableTargetNode`] to match upstream's coverage.
//!
//! Ruby's regex-related backreferences (`$~`, `$&`, `` $` ``, `$'`, `$+`,
//! `$1`..`$9`) are `BackReferenceReadNode`/`NumberedReferenceReadNode` in
//! Prism rather than a global-variable node kind, so they never reach
//! `enter`; this matches upstream, which never reports them either (they
//! are listed in `BUILT_IN_VARS` defensively but the check would allow
//! them regardless).

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, Rule, RuleMeta,
    RuleOptions, Severity, Stability,
};
use ruby_ast::{LocationExt as _, Node, NodeExt as _, NodeKind};
use ruby_source::Span;

const MSG: &str = "Do not introduce global variables.";

/// Built-in global variables and their English aliases, allowed by default.
/// <https://www.zenspider.com/ruby/quickref.html>
const BUILT_IN_VARS: &[&[u8]] = &[
    b"$:",
    b"$LOAD_PATH",
    b"$\"",
    b"$LOADED_FEATURES",
    b"$0",
    b"$PROGRAM_NAME",
    b"$!",
    b"$ERROR_INFO",
    b"$@",
    b"$ERROR_POSITION",
    b"$;",
    b"$FS",
    b"$FIELD_SEPARATOR",
    b"$,",
    b"$OFS",
    b"$OUTPUT_FIELD_SEPARATOR",
    b"$/",
    b"$RS",
    b"$INPUT_RECORD_SEPARATOR",
    b"$\\",
    b"$ORS",
    b"$OUTPUT_RECORD_SEPARATOR",
    b"$.",
    b"$NR",
    b"$INPUT_LINE_NUMBER",
    b"$_",
    b"$LAST_READ_LINE",
    b"$>",
    b"$DEFAULT_OUTPUT",
    b"$<",
    b"$DEFAULT_INPUT",
    b"$$",
    b"$PID",
    b"$PROCESS_ID",
    b"$?",
    b"$CHILD_STATUS",
    b"$~",
    b"$LAST_MATCH_INFO",
    b"$=",
    b"$IGNORECASE",
    b"$*",
    b"$ARGV",
    b"$&",
    b"$MATCH",
    b"$`",
    b"$PREMATCH",
    b"$'",
    b"$POSTMATCH",
    b"$+",
    b"$LAST_PAREN_MATCH",
    b"$stdin",
    b"$stdout",
    b"$stderr",
    b"$DEBUG",
    b"$FILENAME",
    b"$VERBOSE",
    b"$SAFE",
    b"$-0",
    b"$-a",
    b"$-d",
    b"$-F",
    b"$-i",
    b"$-I",
    b"$-l",
    b"$-p",
    b"$-v",
    b"$-w",
    b"$CLASSPATH",
    b"$JRUBY_VERSION",
    b"$JRUBY_REVISION",
    b"$ENV_JAVA",
];

/// Do not introduce global variables.
#[derive(Debug, Clone)]
pub struct GlobalVars {
    allowed_variables: Vec<String>,
}

impl GlobalVars {
    fn allowed_var(&self, name: &[u8]) -> bool {
        BUILT_IN_VARS.contains(&name) || self.allowed_variables.iter().any(|v| v.as_bytes() == name)
    }

    /// RuboCop's `check`: `add_offense(node.loc.name) unless allowed_var?(node.name)`.
    fn check(&self, ctx: &mut Context<'_>, span: Span) {
        let name = ctx.text(span);
        if !self.allowed_var(name) {
            ctx.report(&Self::META, span, MSG);
        }
    }
}

impl Rule for GlobalVars {
    const META: RuleMeta = RuleMeta {
        name: "Style/GlobalVars",
        department: Department::Style,
        summary: "Do not introduce global variables.",
        explanation: "\
Looks for uses of global variables. Global variables introduce
shared mutable state that makes code harder to test, debug,
and reason about, since any part of the program can read or modify them.

It does not report offenses for built-in global variables.
Built-in global variables are allowed by default. Additionally
users can allow additional variables via the AllowedVariables option.

Note that backreferences like $1, $2, etc are not global variables.

```ruby
# bad
$foo = 2
bar = $foo + 5

# good
FOO = 2
foo = 2
$stdin.read
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[
            NodeKind::GlobalVariableReadNode,
            NodeKind::GlobalVariableWriteNode,
            NodeKind::GlobalVariableOrWriteNode,
            NodeKind::GlobalVariableAndWriteNode,
            NodeKind::GlobalVariableOperatorWriteNode,
            NodeKind::GlobalVariableTargetNode,
        ],
        config: &[ConfigOption {
            name: "AllowedVariables",
            default: ConfigDefault::StrList(&[]),
            allowed: &[],
            doc: "Allowed global variables.",
        }],
        blind_spots: "",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        Ok(Self { allowed_variables: options.str_list("AllowedVariables") })
    }

    fn enter(&mut self, node: &Node<'_>, ctx: &mut Context<'_>) {
        match node.kind() {
            NodeKind::GlobalVariableReadNode | NodeKind::GlobalVariableTargetNode => {
                self.check(ctx, node.span());
            }
            NodeKind::GlobalVariableWriteNode => {
                if let Some(w) = node.as_global_variable_write_node() {
                    self.check(ctx, w.name_loc().span());
                }
            }
            NodeKind::GlobalVariableOrWriteNode => {
                if let Some(w) = node.as_global_variable_or_write_node() {
                    self.check(ctx, w.name_loc().span());
                }
            }
            NodeKind::GlobalVariableAndWriteNode => {
                if let Some(w) = node.as_global_variable_and_write_node() {
                    self.check(ctx, w.name_loc().span());
                }
            }
            NodeKind::GlobalVariableOperatorWriteNode => {
                if let Some(w) = node.as_global_variable_operator_write_node() {
                    self.check(ctx, w.name_loc().span());
                }
            }
            _ => {}
        }
    }
}
