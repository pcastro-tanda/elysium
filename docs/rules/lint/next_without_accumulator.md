# Lint/NextWithoutAccumulator

Don't omit the accumulator when calling `next` in a `reduce` block.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Don't omit the accumulator when calling `next` in a `reduce` block.

```ruby
# bad
result = (1..4).reduce(0) do |acc, i|
  next if i.odd?
  acc + i
end

# good
result = (1..4).reduce(0) do |acc, i|
  next acc if i.odd?
  acc + i
end
```

## Options

This rule has no options.

## Blind spots

Only fires when the `reduce`/`inject` call carries exactly one, non-symbol
argument (the initial accumulator value) -- a bare `.reduce { ... }` with no
initial value, or the symbol-operator form `.reduce(:+)`, is never analyzed,
matching upstream's own node-pattern arity requirement. The block's body
must hold at least two statements before it is analyzed at all -- a block
whose only statement is a bare `next` (e.g. `.reduce(0) { |acc, i| next if
i.odd? }`) is never flagged, reproducing a quirk of whitequark's parser
(which only wraps multi-statement bodies in a `begin` node, the shape
upstream's own pattern explicitly requires) that has no Prism equivalent to
suppress. Unlike upstream (which aliases `on_block`/`on_numblock` but not
`on_itblock`), a bare-`it`-parameter block is flagged here too, since Prism
represents all three block forms identically. A `next` nested inside a
`def` within the block (not itself inside a further nested block) is still
matched against the outer block, mirroring upstream's own ancestor walk,
which does not stop at `def`/`class` boundaries either.
