# Style/MultilineBlockChain

Checks for chaining of a block after another block that spans multiple lines.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks for chaining of a block after another block that spans multiple
lines.

```ruby
# bad
Thread.list.select do |t|
  t.alive?
end.map do |t|
  t.object_id
end

# good
alive_threads = Thread.list.select do |t|
  t.alive?
end
alive_threads.map do |t|
  t.object_id
end
```

## Options

This rule has no options.

## Blind spots

Only the immediate receiver chain of the call that owns a block is walked
for a multi-line block-shaped receiver; upstream's `each_node(:call)` would
also search argument subtrees once the whole receiver chain is exhausted,
so a block-shaped receiver reachable only through an argument (never
through any level of the receiver chain) is not flagged. No fixture in the
corpus exercises this.
