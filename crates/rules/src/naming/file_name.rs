//! `Naming/FileName`, ported from RuboCop's
//! `lib/rubocop/cop/naming/file_name.rb`.
//!
//! This cop never inspects the node it is subscribed to (`kinds: &[]`);
//! everything happens in [`Rule::file_end`], mirroring `Bundler/GemFilename`
//! and `Lint/EmptyFile`'s file-level shape. The `Exclude`/camel-case-file
//! allowances upstream implements via `config.file_to_exclude?` /
//! `config.allowed_camel_case_file?` are ordinary per-cop `Exclude` path
//! matching, already handled by the engine before a rule ever runs, so
//! neither is reimplemented here.
//!
//! `ExpectMatchingDefinition` walks the whole file looking for a `class`,
//! `module`, or `NAME = Struct.new` whose name matches the file's expected
//! namespace, mirroring upstream's `find_class_or_module` /
//! `namespace_matches?`: each candidate definition's own (possibly compound,
//! `A::B`-style) qualifying name plus its lexical ancestors' qualifying
//! names are peeled, innermost first, against the expected namespace stack.
//! A `class << self` ancestor always disqualifies a candidate, matching
//! upstream's `return false if ancestor.sclass_type?`.

use linter::{
    ConfigDefault, ConfigOption, Context, Department, FixAvailability, OptionError, OptionValue,
    Rule, RuleMeta, RuleOptions, Severity, Stability,
};
use regex::Regex;
use ruby_ast::{Node, NodeExt as _, NodeKind};

/// RuboCop's `AllowedAcronyms` default (`config/default.yml`).
const DEFAULT_ACRONYMS: &[&str] = &[
    "CLI", "DSL", "ACL", "API", "ASCII", "CPU", "CSS", "DNS", "EOF", "GUID", "HTML", "HTTP",
    "HTTPS", "ID", "IP", "JSON", "LHS", "QPS", "RAM", "RHS", "RPC", "SLA", "SMTP", "SQL", "SSH",
    "TCP", "TLS", "TTL", "UDP", "UI", "UID", "UUID", "URI", "URL", "UTF8", "VM", "XML", "XMPP",
    "XSRF", "XSS",
];

/// RuboCop's `CheckDefinitionPathHierarchyRoots` default.
const DEFAULT_ROOTS: &[&str] = &["lib", "spec", "test", "src"];

/// `AllCops:Include` patterns (`config/default.yml`) that contain an
/// uppercase letter -- an exact basename match, except `**/*Fastfile`
/// (any basename ending in `Fastfile`). RuboCop's `allowed_camel_case_file?`
/// exempts a file matching any of these (plus `.gemspec`, checked
/// separately) from the `snake_case` check entirely.
const CAMEL_CASE_INCLUDE_EXACT: &[&str] = &[
    "Appraisals",
    "Berksfile",
    "Brewfile",
    "Buildfile",
    "Capfile",
    "Dangerfile",
    "Deliverfile",
    "Fastfile",
    "Gemfile",
    "Guardfile",
    "Jarfile",
    "Mavenfile",
    "Podfile",
    "Puppetfile",
    "Rakefile",
    "Schemafile",
    "Snapfile",
    "Steepfile",
    "Thorfile",
    "Vagrantfile",
];

/// RuboCop's `allowed_camel_case_file?`.
fn allowed_camel_case_file(basename: &str) -> bool {
    basename.ends_with(".gemspec")
        || CAMEL_CASE_INCLUDE_EXACT.contains(&basename)
        || basename.ends_with("Fastfile")
}

/// One `class`/`module`/`NAME = Struct.new` definition found anywhere in the
/// file: its own qualifying name, split into the compound prefix (e.g. `P`
/// in `module P::Q`, empty for a simple name) and the final segment, plus
/// its lexical ancestors, innermost first.
struct Candidate {
    prefix: Vec<String>,
    name: String,
    ancestors: Vec<Frame>,
}

/// One active lexical scope while walking the tree: a named `class`/
/// `module`/struct assignment (its own full qualifying name, outer to
/// inner), or a singleton class (`class << self`), which upstream's
/// `namespace_matches?` treats as an automatic non-match.
#[derive(Clone)]
enum Frame {
    Named(Vec<String>),
    Sclass,
}

/// Use `snake_case` for source file names.
#[derive(Debug, Clone)]
pub struct FileName {
    expect_matching_definition: bool,
    check_definition_path_hierarchy: bool,
    roots: Vec<String>,
    regex: Option<Regex>,
    regex_raw: Option<String>,
    ignore_executable_scripts: bool,
    allowed_acronyms: Vec<String>,
}

impl Rule for FileName {
    const META: RuleMeta = RuleMeta {
        name: "Naming/FileName",
        department: Department::Naming,
        summary: "Use snake_case for source file names.",
        explanation: "\
Makes sure that Ruby source files have snake_case names. Ruby scripts
(i.e. source files with a shebang in the first line) are ignored.

The cop also ignores `.gemspec` files, because Bundler recommends using
dashes to separate namespaces in nested gems (i.e. `bundler-console`
becomes `Bundler::Console`). As such, the gemspec is supposed to be named
`bundler-console.gemspec`.

When `ExpectMatchingDefinition` (default: `false`) is `true`, the cop
requires each file to have a class, module or `Struct` defined in it that
matches the filename. This can be further configured using
`CheckDefinitionPathHierarchy` (default: `true`) to determine whether the
path should match the namespace of the above definition.

When `IgnoreExecutableScripts` (default: `true`) is `true`, files that
start with a shebang line are not considered by the cop.

When `Regex` is set, the cop will flag any filename that does not match
the regular expression.

```ruby
# bad
lib/layoutManager.rb

anything/usingCamelCase

# good
lib/layout_manager.rb

anything/using_snake_case.rake
```",
        enabled_by_default: true,
        severity: Severity::Convention,
        fix: FixAvailability::None,
        stability: Stability::Stable,
        kinds: &[],
        config: &[
            ConfigOption {
                name: "ExpectMatchingDefinition",
                default: ConfigDefault::Bool(false),
                allowed: &[],
                doc: "Requires each source file to define a class or module matching the \
                      filename.",
            },
            ConfigOption {
                name: "CheckDefinitionPathHierarchy",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether the expected namespace must also match the directory hierarchy.",
            },
            ConfigOption {
                name: "CheckDefinitionPathHierarchyRoots",
                default: ConfigDefault::StrList(DEFAULT_ROOTS),
                allowed: &[],
                doc: "Path components considered root directories for namespace hierarchy \
                      checks.",
            },
            ConfigOption {
                name: "Regex",
                default: ConfigDefault::Nil,
                allowed: &[],
                doc: "If set, source file names must match this regular expression.",
            },
            ConfigOption {
                name: "IgnoreExecutableScripts",
                default: ConfigDefault::Bool(true),
                allowed: &[],
                doc: "Whether files starting with a shebang line are ignored.",
            },
            ConfigOption {
                name: "AllowedAcronyms",
                default: ConfigDefault::StrList(DEFAULT_ACRONYMS),
                allowed: &[],
                doc: "Acronyms allowed to appear uppercased in an otherwise snake_case name.",
            },
        ],
        blind_spots: "\
`ConstantPathWriteNode` (a compound `A::B = Struct.new` assignment) is not
recognized as a struct definition; upstream's own `def_node_matcher`
pattern only matches a plain `casgn`, so this mirrors that.",
    };

    fn configure(options: &RuleOptions) -> Result<Self, OptionError> {
        let regex_raw = options
            .get("Regex")
            .and_then(OptionValue::as_str)
            .map(std::string::ToString::to_string);
        let regex = match &regex_raw {
            Some(pattern) => Some(Regex::new(pattern).map_err(|error| OptionError {
                rule: Self::META.name,
                option: "Regex".to_string(),
                message: error.to_string(),
            })?),
            None => None,
        };
        Ok(Self {
            expect_matching_definition: options.bool("ExpectMatchingDefinition"),
            check_definition_path_hierarchy: options.bool("CheckDefinitionPathHierarchy"),
            roots: options.str_list("CheckDefinitionPathHierarchyRoots"),
            regex,
            regex_raw,
            ignore_executable_scripts: options.bool("IgnoreExecutableScripts"),
            allowed_acronyms: options.str_list("AllowedAcronyms"),
        })
    }

    fn file_end(&mut self, ctx: &mut Context<'_>) {
        let path = ctx.source().path();
        let Some(basename) = path.file_name().map(|name| name.to_string_lossy().into_owned())
        else {
            return;
        };
        if allowed_camel_case_file(&basename) {
            return;
        }
        let file_path = path.to_string_lossy().into_owned();

        let msg = if self.filename_good(&basename) {
            self.perform_class_and_module_naming_checks(ctx, &file_path, &basename)
        } else if self.bad_filename_allowed(ctx) {
            None
        } else {
            Some(self.other_message(&basename))
        };

        if let Some(msg) = msg {
            ctx.report_global(&Self::META, msg);
        }
    }
}

impl FileName {
    /// RuboCop's `bad_filename_allowed?`.
    fn bad_filename_allowed(&self, ctx: &Context<'_>) -> bool {
        self.ignore_executable_scripts && ctx.source().bytes().starts_with(b"#!")
    }

    /// RuboCop's `filename_good?`.
    fn filename_good(&self, basename: &str) -> bool {
        let stripped = basename.strip_prefix('.').unwrap_or(basename);
        let without_ext = strip_last_extension(stripped);
        let normalized = without_ext.replace('+', "_");
        match &self.regex {
            Some(re) => re.is_match(&normalized),
            None => is_snake_case(&normalized),
        }
    }

    /// RuboCop's `other_message`.
    fn other_message(&self, basename: &str) -> String {
        match &self.regex_raw {
            Some(pattern) => format!("`{basename}` should match `{pattern}`."),
            None => format!("The name of this source file (`{basename}`) should use snake_case."),
        }
    }

    /// RuboCop's `perform_class_and_module_naming_checks`.
    fn perform_class_and_module_naming_checks(
        &self,
        ctx: &Context<'_>,
        file_path: &str,
        basename: &str,
    ) -> Option<String> {
        if !self.expect_matching_definition {
            return None;
        }

        let root = ctx.parsed().root();
        let mut candidates = Vec::new();
        let mut stack = Vec::new();
        collect_candidates(&root, &mut stack, &mut candidates);

        let hierarchy_ns = to_namespace(file_path, &self.roots);
        let cond1 = self.check_definition_path_hierarchy
            && !matches_definition(&candidates, hierarchy_ns.clone(), &self.allowed_acronyms);
        if cond1 {
            return Some(no_definition_message(basename, &hierarchy_ns));
        }

        let basename_ns = to_namespace(basename, &self.roots);
        if !matches_definition(&candidates, basename_ns.clone(), &self.allowed_acronyms) {
            return Some(no_definition_message(basename, &basename_ns));
        }
        None
    }
}

/// RuboCop's `no_definition_message`.
fn no_definition_message(basename: &str, namespace: &[String]) -> String {
    format!("`{basename}` should define a class or module called `{}`.", namespace.join("::"))
}

/// RuboCop's `SNAKE_CASE` regex: `/^[\d[[:lower:]]_.?!]+$/`.
fn is_snake_case(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_digit() || c.is_lowercase() || matches!(c, '_' | '.' | '?' | '!'))
}

/// `basename.sub(/\.[^.]+$/, '')`: strips a trailing extension (the part
/// after the *last* dot), only when that part is non-empty.
fn strip_last_extension(s: &str) -> &str {
    match s.rfind('.') {
        Some(pos) if pos + 1 < s.len() => &s[..pos],
        _ => s,
    }
}

/// RuboCop's `to_module_name`: `basename.sub(/\..*/, '').split('_').map(&:capitalize).join`.
fn to_module_name(basename: &str) -> String {
    let stem = basename.split('.').next().unwrap_or(basename);
    stem.split('_').map(capitalize).collect()
}

/// Ruby's `String#capitalize`: first character uppercased, the rest lowered.
fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars.flat_map(char::to_lowercase)).collect(),
        None => String::new(),
    }
}

/// RuboCop's `to_namespace`: splits `path` into filename components, finds
/// the rightmost component that is a configured hierarchy root, and maps
/// every component after it through [`to_module_name`]; with no root found,
/// falls back to the last component alone.
fn to_namespace(path: &str, roots: &[String]) -> Vec<String> {
    let components: Vec<&str> = path.split('/').filter(|c| !c.is_empty() && *c != ".").collect();
    let mut start_index = None;
    for i in (0..components.len()).rev() {
        if roots.iter().any(|r| r == components[i]) {
            start_index = Some(i + 1);
            break;
        }
    }
    match start_index {
        Some(idx) => components[idx..].iter().copied().map(to_module_name).collect(),
        None => vec![to_module_name(components.last().copied().unwrap_or(""))],
    }
}

/// RuboCop's `match_acronym?`: `name = allowed_acronyms.reduce(name) { |r,
/// acronym| r.gsub(acronym, acronym.capitalize) }; expected == name`. Every
/// acronym is applied in configured order to `actual`, each replacing every
/// occurrence of its literal (upper-case) form with its capitalized form
/// (`String#capitalize`: first letter upper, rest lower) -- so an earlier,
/// coincidentally-overlapping acronym (e.g. `RDO` inside `IRDOauth`) can
/// consume characters before a later, intended one (`IRD`) ever matches.
fn match_acronym(expected: &str, actual: &str, acronyms: &[String]) -> bool {
    let mut name = actual.to_string();
    for acronym in acronyms {
        name = name.replace(acronym.as_str(), &capitalize(acronym));
    }
    expected == name
}

/// RuboCop's `partial_matcher!`'s inner loop body, applied to one candidate
/// or ancestor's qualifying name segments (innermost first): pops
/// `expected`'s last element whenever it (or its allowed-acronym form)
/// equals the next segment.
fn peel<'a>(
    segments_innermost_first: impl Iterator<Item = &'a String>,
    expected: &mut Vec<String>,
    acronyms: &[String],
) {
    for seg in segments_innermost_first {
        if let Some(last) = expected.last() {
            if seg == last || match_acronym(last, seg, acronyms) {
                expected.pop();
            }
        }
    }
}

/// RuboCop's `find_class_or_module` + `namespace_matches?`: `namespace`'s
/// last element is the required final segment; a candidate matches when its
/// own name (or acronym form) equals it and its own compound prefix plus
/// its lexical ancestors' qualifying names, peeled innermost first, exactly
/// consume the remaining `namespace` (or leave only `Object`).
fn matches_definition(
    candidates: &[Candidate],
    mut namespace: Vec<String>,
    acronyms: &[String],
) -> bool {
    let Some(name) = namespace.pop() else { return false };
    for candidate in candidates {
        if !(candidate.name == name || match_acronym(&name, &candidate.name, acronyms)) {
            continue;
        }
        let mut expected = namespace.clone();
        peel(candidate.prefix.iter().rev(), &mut expected, acronyms);

        let mut disqualified = false;
        for frame in candidate.ancestors.iter().rev() {
            match frame {
                Frame::Sclass => {
                    disqualified = true;
                    break;
                }
                Frame::Named(segments) => peel(segments.iter().rev(), &mut expected, acronyms),
            }
        }
        if !disqualified && (expected.is_empty() || expected == ["Object".to_string()]) {
            return true;
        }
    }
    false
}

/// The full qualifying name of a `ConstantReadNode` (a simple name) or
/// `ConstantPathNode` (a compound `A::B` path), outer segment first; a
/// leading `::` (`cbase`, no `parent`) contributes nothing extra.
fn full_segments(node: &Node<'_>) -> Vec<String> {
    match node.kind() {
        NodeKind::ConstantReadNode => node
            .as_constant_read_node()
            .map(|c| vec![String::from_utf8_lossy(c.name().as_slice()).into_owned()])
            .unwrap_or_default(),
        NodeKind::ConstantPathNode => {
            let Some(path) = node.as_constant_path_node() else { return Vec::new() };
            let mut segments = match path.parent() {
                Some(parent) => full_segments(&parent),
                None => Vec::new(),
            };
            if let Some(name) = path.name() {
                segments.push(String::from_utf8_lossy(name.as_slice()).into_owned());
            }
            segments
        }
        _ => Vec::new(),
    }
}

/// RuboCop's `struct_definition` matcher's receiver check: `(const {nil?
/// cbase} :Struct)` -- a bare `Struct` or an explicit `::Struct`, not e.g.
/// `Foo::Struct`.
fn is_struct_receiver(node: &Node<'_>) -> bool {
    match node.kind() {
        NodeKind::ConstantReadNode => {
            node.as_constant_read_node().is_some_and(|c| c.name().as_slice() == b"Struct")
        }
        NodeKind::ConstantPathNode => node.as_constant_path_node().is_some_and(|path| {
            path.parent().is_none() && path.name().is_some_and(|n| n.as_slice() == b"Struct")
        }),
        _ => false,
    }
}

/// RuboCop's `struct_definition` matcher's value pattern: `(send (const
/// {nil? cbase} :Struct) :new ...)`, with or without an attached block --
/// in Prism a block is embedded in the `CallNode` itself.
fn is_struct_new_value(node: &Node<'_>) -> bool {
    node.as_call_node().is_some_and(|call| {
        call.name().as_slice() == b"new" && call.receiver().is_some_and(|r| is_struct_receiver(&r))
    })
}

/// Records `segments` (outer to inner) as one [`Candidate`]: everything but
/// the last element is its compound prefix, the last element its own name.
fn push_candidate(segments: &[String], stack: &[Frame], out: &mut Vec<Candidate>) {
    let Some((name, prefix)) = segments.split_last() else { return };
    out.push(Candidate { prefix: prefix.to_vec(), name: name.clone(), ancestors: stack.to_vec() });
}

/// Walks the whole tree (matching upstream's `on_node` over every `class`/
/// `module`/`casgn`, regardless of nesting) collecting every candidate
/// definition, while tracking the lexical scope stack a definition -- or
/// anything nested inside a `class`/`module`/struct-assignment/singleton
/// class -- currently sits in.
fn collect_candidates(node: &Node<'_>, stack: &mut Vec<Frame>, out: &mut Vec<Candidate>) {
    match node.kind() {
        NodeKind::ClassNode => {
            let Some(class) = node.as_class_node() else { return };
            let segments = full_segments(&class.constant_path());
            push_candidate(&segments, stack, out);
            stack.push(Frame::Named(segments));
            ruby_ast::for_each_child(node, |child| collect_candidates(child, stack, out));
            stack.pop();
        }
        NodeKind::ModuleNode => {
            let Some(module) = node.as_module_node() else { return };
            let segments = full_segments(&module.constant_path());
            push_candidate(&segments, stack, out);
            stack.push(Frame::Named(segments));
            ruby_ast::for_each_child(node, |child| collect_candidates(child, stack, out));
            stack.pop();
        }
        NodeKind::SingletonClassNode => {
            stack.push(Frame::Sclass);
            ruby_ast::for_each_child(node, |child| collect_candidates(child, stack, out));
            stack.pop();
        }
        NodeKind::ConstantWriteNode => {
            let Some(write) = node.as_constant_write_node() else { return };
            if is_struct_new_value(&write.value()) {
                let segments = vec![String::from_utf8_lossy(write.name().as_slice()).into_owned()];
                push_candidate(&segments, stack, out);
                stack.push(Frame::Named(segments));
                ruby_ast::for_each_child(node, |child| collect_candidates(child, stack, out));
                stack.pop();
            } else {
                ruby_ast::for_each_child(node, |child| collect_candidates(child, stack, out));
            }
        }
        _ => ruby_ast::for_each_child(node, |child| collect_candidates(child, stack, out)),
    }
}
