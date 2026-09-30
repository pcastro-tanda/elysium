# Layout/BlockAlignment

Align block ends correctly.

| | |
| --- | --- |
| Department | Layout |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Checks whether the `end` keywords of `do`/`end` blocks (and the closing `}`
of brace blocks) are aligned properly, for the blocks whose `end` begins its
own line.

```ruby
# bad
foo.bar
  .each do
    baz
      end

# good (EnforcedStyleAlignWith: either, the default -- both are accepted)
foo.bar
  .each do
    baz
  end

foo.bar
  .each do
    baz
end
```

`start_of_block` requires alignment with the start of the line the `do` is
on, `start_of_line` with the start of the line the whole expression started
on. When the `do` or `{` sits on a continuation line of a parenthesised
argument list, the method dispatch line is the anchor instead of that
continuation line.

## Options

| Name | Default | Allowed values | Description |
| --- | --- | --- | --- |
| EnforcedStyleAlignWith | `either` | `either`, `start_of_block`, `start_of_line` | Which line the block's `end` must line up with. |

## Blind spots

`inside_parentheses?` counts raw `(`/`[`/`)`/`]` bytes outside string,
symbol, regexp and comment bodies rather than lexer tokens, so a `(`
delimiting a bare command argument list (`foo (a,` -- upstream's
`tLPAREN_ARG`, which it does not count) and the brackets of a `%w[...]`
literal are counted where upstream would not.
