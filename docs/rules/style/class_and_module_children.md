# Style/ClassAndModuleChildren

Checks that namespaced classes and modules are defined with a consistent style.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | unsafe |
| Stability | stable |

With `nested` style, classes and modules should be defined separately (one
constant on each line, without `::`). With `compact` style, classes and
modules should be defined with fully qualified names (using `::` for
namespaces). The compact style is only forced for classes/modules with one
child.

By default `EnforcedStyle` applies to both classes and modules; separate
styles can be set with `EnforcedStyleForClasses`/`EnforcedStyleForModules`.

```ruby
# EnforcedStyle: nested (default)
# good
class Foo
  class Bar
  end
end

# EnforcedStyle: compact
# good
class Foo::Bar
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `nested` | `nested`, `compact` | Whether children should be nested (one per line) or compacted (`A::B`). |
| EnforcedStyleForClasses | `nil` | `nested`, `compact` | Overrides `EnforcedStyle` for classes only, when set. |
| EnforcedStyleForModules | `nil` | `nested`, `compact` | Overrides `EnforcedStyle` for modules only, when set. |

## Blind spots

- `AlignmentCorrector`'s heredoc/delimited-string-literal protection
  (`inside_string_ranges`) and `=begin`/`=end` block-comment guard
  (`block_comment_within?`) are not ported: an unindent shift touching a
  heredoc body or straddling a block comment could mis-edit it. No fixture
  exercises this.
- `AllCops/UseProjectIndex`/the `rubydex` gem's project-wide index (upstream's
  `ProjectIndexHelp`) is not ported: `namespace_keyword` always falls back to
  the heuristic left-sibling `class`/`module` search, and a compacting fix is
  always offered without verifying the namespace is defined elsewhere,
  matching upstream's own behavior when the index is unavailable.
- Compacting (or splitting) a name with two or more `::` levels only
  resolves one level per lint pass, matching upstream's own single-node
  `add_offense` (only the outermost namespace is flagged per pass); the
  fixture harness's iterate-to-fixpoint autocorrect loop resolves the rest.
