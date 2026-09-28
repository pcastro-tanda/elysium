# Naming/FileName

Use snake_case for source file names.

| | |
| --- | --- |
| Department | Naming |
| Enabled by default | true |
| Default severity | convention |
| Fix | none |
| Stability | stable |

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
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| ExpectMatchingDefinition | false |  | Requires each source file to define a class or module matching the filename. |
| CheckDefinitionPathHierarchy | true |  | Whether the expected namespace must also match the directory hierarchy. |
| CheckDefinitionPathHierarchyRoots | `lib`, `spec`, `test`, `src` |  | Path components considered root directories for namespace hierarchy checks. |
| Regex | `nil` |  | If set, source file names must match this regular expression. |
| IgnoreExecutableScripts | true |  | Whether files starting with a shebang line are ignored. |
| AllowedAcronyms | `CLI`, `DSL`, `ACL`, `API`, `ASCII`, `CPU`, `CSS`, `DNS`, `EOF`, `GUID`, `HTML`, `HTTP`, `HTTPS`, `ID`, `IP`, `JSON`, `LHS`, `QPS`, `RAM`, `RHS`, `RPC`, `SLA`, `SMTP`, `SQL`, `SSH`, `TCP`, `TLS`, `TTL`, `UDP`, `UI`, `UID`, `UUID`, `URI`, `URL`, `UTF8`, `VM`, `XML`, `XMPP`, `XSRF`, `XSS` |  | Acronyms allowed to appear uppercased in an otherwise snake_case name. |

## Blind spots

`ConstantPathWriteNode` (a compound `A::B = Struct.new` assignment) is not
recognized as a struct definition; upstream's own `def_node_matcher`
pattern only matches a plain `casgn`, so this mirrors that.
