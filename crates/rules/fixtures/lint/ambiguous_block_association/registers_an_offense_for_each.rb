foo bar.each do |x|
    ^^^^^^^^ `each` is called without a block because the `do` block binds to `foo`. Use braces or extract to a variable.
  x
end
