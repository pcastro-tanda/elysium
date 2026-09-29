map do |a|
    ^^ Prefer `{...}` over `do...end` for multi-line chained blocks.
  do_something
rescue StandardError => e
  puts 'oh no'
end.join('-')
