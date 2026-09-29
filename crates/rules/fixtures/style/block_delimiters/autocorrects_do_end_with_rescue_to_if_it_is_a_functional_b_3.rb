map do |a|
    ^^ Prefer `{...}` over `do...end` for blocks.
  do_something
rescue StandardError => e
  puts 'oh no'
end
