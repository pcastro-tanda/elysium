# Lint/NonLocalExitFromIterator

Checks for non-local exits from iterators without a return value.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | none |
| Stability | nursery |

It registers an offense under these conditions:

* No value is returned,
* the block is preceded by a method chain,
* the block has arguments,
* the method which receives the block is not `define_method`
  or `define_singleton_method`,
* the return is not contained in an inner scope, e.g. a lambda or a
  method definition.

```ruby
class ItemApi
  rescue_from ValidationError do |e| # non-iteration block with arg
    return { message: 'validation error' } unless e.errors # allowed
    error_array = e.errors.map do |error| # block with method chain
      return if error.suppress? # warned
      return "#{error.param}: invalid" unless error.message # allowed
      "#{error.param}: #{error.message}"
    end
    { message: 'validation error', errors: error_array }
  end

  def update_items
    transaction do # block without arguments
      return unless update_necessary? # allowed
      find_each do |item| # block without method chain
        return if item.stock == 0 # false-negative...
        item.update!(foobar: true)
      end
    end
  end
end
```

## Options

This rule has no options.

## Blind spots

Upstream's ancestor walk stops looking further out the moment it passes an
argument-less block, *unless* that block is itself scoped or a
`define_method`/`lambda` call -- so a chained, argumented block nested
inside an argument-less, unchained one (`transaction do; find_each do |item|;
return if ...; end; end`) is a documented false-negative on both sides of
this port (the `find_each` block itself is not chained, and the search never
reaches past `transaction`'s empty argument list to find a chained ancestor
further out, because there is none in that example -- see the last spec
example). `chained_send?`/`lambda?`/`define_method?` all match by bare
method name only, with no receiver check beyond `chained_send?`'s own
non-nil requirement, exactly like upstream.
