# Lint/StructNewOverride

Checks unexpected overrides of the `Struct` built-in methods via `Struct.new`.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

Checks unexpected overrides of the `Struct` built-in methods
via `Struct.new`.

```ruby
# bad
Bad = Struct.new(:members, :clone, :count)
b = Bad.new([], true, 1)
b.members #=> [] (overriding `Struct#members`)
b.clone #=> true (overriding `Object#clone`)
b.count #=> 1 (overriding `Enumerable#count`)

# good
Good = Struct.new(:id, :name)
g = Good.new(1, "foo")
g.members #=> [:id, :name]
g.clone #=> #<struct Good id=1, name="foo">
g.count #=> 2
```

## Options

This rule has no options.

## Blind spots

`STRUCT_METHOD_NAMES` is a hardcoded snapshot of `Struct.instance_methods` on Ruby 3.4.2 (see the
module doc) rather than a live computation; a member name that is a `Struct` instance method only
on some other Ruby version is not detected.
