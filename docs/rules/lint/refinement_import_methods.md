# Lint/RefinementImportMethods

Use `Refinement#import_methods` when using `include` or `prepend` in `refine` block.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | false |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

These methods are deprecated and should be replaced with `Refinement#import_methods`.

It emulates deprecation warnings in Ruby 3.1. Functionality has been removed in Ruby 3.2.

@safety
  This cop's autocorrection is unsafe because `include M` will affect the included class
  if any changes are made to module `M`.
  On the other hand, `import_methods M` uses a snapshot of method definitions,
  thus it will not be affected if module `M` changes.

```ruby
# bad
refine Foo do
  include Bar
end

# bad
refine Foo do
  prepend Bar
end

# good
refine Foo do
  import_methods Bar
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
