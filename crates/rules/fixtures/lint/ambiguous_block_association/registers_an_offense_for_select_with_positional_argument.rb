foo bar.select do |x|
    ^^^^^^^^^^ `select` is called without a block because the `do` block binds to `foo`. Use braces or extract to a variable.
  x
end
