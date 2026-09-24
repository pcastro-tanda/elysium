# Layout/IndentationConsistency

Keep indentation straight.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks that entities at the same logical depth share the same indentation.
The `indented_internal_methods` style additionally requires that a bare
`protected`/`private`/`public`/`module_function` marker stay flush with the
surrounding methods, while the members below it are indented one step
further than the marker.

```ruby
# EnforcedStyle: normal (default)

# bad
class A
  def test
    puts 'hello'
     puts 'world'
  end
end

# good
class A
  def test
    puts 'hello'
    puts 'world'
  end

  protected

  def foo
  end
end
```

```ruby
# EnforcedStyle: indented_internal_methods

# bad
class A
  def test
    puts 'hello'
     puts 'world'
  end
end

# good
class A
  def test
    puts 'hello'
    puts 'world'
  end

  protected

    def foo
    end
end
```

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyle | `normal` | `normal`, `indented_internal_methods` | Whether `protected`/`private` markers must be indented like the surrounding methods (`normal`) or may sit flush with a deeper-indented internal section (`indented_internal_methods`). |

## Blind spots

`bare_access_modifier?` does not replicate RuboCop's recursive
`in_macro_scope?` check (it does not confirm the call sits directly in a
class/module/top-level body through only `begin`/`kwbegin`/block/`if`
wrappers): any receiver-less, argument-less `public`/`protected`/`private`/
`module_function` call is treated as a group divider or excluded from
alignment, even one nested inside a `def` or loop body. That only narrows a
comparison into smaller groups (or drops one item entirely), never merges
two real groups into one, so it can only produce false negatives.
`private()`/`protected()` with empty parentheses are not recognized as bare
modifiers (RuboCop's AST does not distinguish them from a no-args call, but
our simpler receiver/arguments check does), which is a false-negative-only
divergence in the same direction.

Autocorrection's taboo-range protection (RuboCop's `AlignmentCorrector`
`inside_string_ranges`) only covers heredoc bodies; the interior of an
ordinary multi-line quoted string or `%`-literal that itself begins a
physical line inside a misaligned body is not separately protected. The
block-comment guard is a per-line `=begin` text match rather than resolving
actual `EmbDoc` comment nodes, matching this crate's other `Alignment`-based
cops.
