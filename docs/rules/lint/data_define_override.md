# Lint/DataDefineOverride

Disallow overriding the `Data` built-in methods via `Data.define`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | none |
| Stability | stable |

Checks unexpected overrides of the `Data` built-in methods via `Data.define`.

```ruby
# bad
Bad = Data.define(:members, :clone, :to_s)
b = Bad.new(members: [], clone: true, to_s: 'bad')
b.members #=> [] (overriding `Data#members`)
b.clone #=> true (overriding `Object#clone`)
b.to_s #=> "bad" (overriding `Data#to_s`)

# good
Good = Data.define(:id, :name)
g = Good.new(id: 1, name: "foo")
g.members #=> [:id, :name]
g.clone #=> #<data Good id=1, name="foo">
```

## Options

This rule has no options.

## Blind spots

None recorded.
