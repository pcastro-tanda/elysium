if condition
  foo
else
  begin
  ^^^^^ Redundant `begin` block detected.
    bar
    baz
  end
end
