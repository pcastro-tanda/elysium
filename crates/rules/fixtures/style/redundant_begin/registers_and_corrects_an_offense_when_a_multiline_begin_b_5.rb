case condition
  when foo
    begin
    ^^^^^ Redundant `begin` block detected.
      bar
      baz
    end
end
