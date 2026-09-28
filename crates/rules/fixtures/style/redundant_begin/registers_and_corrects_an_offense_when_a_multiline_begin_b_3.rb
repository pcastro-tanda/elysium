if condition
  foo
elsif condition2
  begin
  ^^^^^ Redundant `begin` block detected.
    bar
    baz
  end
end
