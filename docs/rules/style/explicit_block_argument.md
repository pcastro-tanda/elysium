# Style/ExplicitBlockArgument

Consider using explicit block argument to avoid writing block literal that just passes its arguments to another block.

| | |
| --- | --- |
| Department | Style |
| Enabled by default | true |
| Default severity | convention |
| Fix | safe |
| Stability | stable |

Enforces the use of explicit block argument to avoid writing
block literal that just passes its arguments to another block.

NOTE: This cop only registers an offense if the block args match the
yield args exactly.

```ruby
# bad
def with_tmp_dir
  Dir.mktmpdir do |tmp_dir|
    Dir.chdir(tmp_dir) { |dir| yield dir } # block just passes arguments
  end
end

# bad
def nine_times
  9.times { yield }
end

# good
def with_tmp_dir(&block)
  Dir.mktmpdir do |tmp_dir|
    Dir.chdir(tmp_dir, &block)
  end
end

with_tmp_dir do |dir|
  puts "dir is accessible as a parameter and pwd is set: #{dir}"
end

# good
def nine_times(&block)
  9.times(&block)
end
```

## Options

This rule has no options.

## Blind spots

The block/yield argument match is a naive by-name comparison (upstream's own
`children.first` equality), ported faithfully: a destructured block
parameter, `...` forwarding, or an anonymous `*`/`**`/`&` parameter never
matches any yielded expression, even when the intent is equivalent. A
`def`'s own anonymous block-forwarding parameter (`def m(&)`) is treated as
having no existing name, so its extracted block name falls back to the
literal `block` rather than reusing the anonymous form.
