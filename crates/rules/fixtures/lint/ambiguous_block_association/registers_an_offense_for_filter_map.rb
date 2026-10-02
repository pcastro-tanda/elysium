foo bar.filter_map do |x|
    ^^^^^^^^^^^^^^ `filter_map` is called without a block because the `do` block binds to `foo`. Use braces or extract to a variable.
  x
end
