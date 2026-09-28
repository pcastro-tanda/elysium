case condition
  when foo
    bar
  else
    begin
    ^^^^^ Redundant `begin` block detected.
      baz
      quux
    end
end
