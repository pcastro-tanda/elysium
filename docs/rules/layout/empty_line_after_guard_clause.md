# Layout/EmptyLineAfterGuardClause

Add empty line after guard clause.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks for guard clauses (`return`/`break`/`next`/`raise`/`fail`, possibly
`and`/`or`-wrapped) not followed by an empty line.

```ruby
# bad
def foo
  return if need_return?
  bar
end

# good
def foo
  return if need_return?

  bar
end

# good
def foo
  return if something?
  return if something_different?

  bar
end

# also good
def foo
  if something?
    do_something
    return if need_return?
  end
end
```

A SimpleCov directive comment (`# :nocov:` or `# simplecov:disable`/
`# simplecov:enable`) directly after the guard clause is allowed, since
SimpleCov excludes code from the coverage report by wrapping it in such
directives.

## Options

This rule has no options.

## Blind spots

`node.parent&.assignment?`-style whitequark-parent quirks (a single-statement
`if`/`unless` branch reaching its own `elsif`/`else`; a `rescue`/`ensure`
main body reaching the `rescue`/`ensure` node) are not walked explicitly --
see the module doc comment for why the plain next-sibling check this port
uses instead always agrees with them.
