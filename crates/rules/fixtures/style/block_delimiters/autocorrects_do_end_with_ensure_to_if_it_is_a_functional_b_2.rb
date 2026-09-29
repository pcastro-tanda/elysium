map do |a|
    ^^ Prefer `{...}` over `do...end` for multi-line chained blocks.
  do_something
ensure
  puts 'oh no'
end.join('-')
