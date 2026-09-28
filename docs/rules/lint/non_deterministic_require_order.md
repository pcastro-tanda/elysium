# Lint/NonDeterministicRequireOrder

Always sort arrays returned by Dir.glob when requiring files.

| | |
| --- | --- |
| Department | Lint |
| Enabled by default | true |
| Default severity | warning |
| Fix | unsafe |
| Stability | stable |

`Dir[...]` and `Dir.glob(...)` do not make any guarantees about the order in which files are returned. The final order is determined by the operating system and file system. This means that using them in cases where the order matters, such as requiring files, can lead to intermittent failures that are hard to debug. To ensure this doesn't happen, always sort the list.

`Dir.glob` and `Dir[]` sort globbed results by default in Ruby 3.0. So all bad cases are acceptable when Ruby 3.0 or higher are used.

NOTE: This cop will be deprecated and removed when supporting only Ruby 3.0 and higher.

```ruby
# bad
Dir["./lib/**/*.rb"].each do |file|
  require file
end

# good
Dir["./lib/**/*.rb"].sort.each do |file|
  require file
end
```

## Options

This rule has no options.

## Blind spots

None recorded.
