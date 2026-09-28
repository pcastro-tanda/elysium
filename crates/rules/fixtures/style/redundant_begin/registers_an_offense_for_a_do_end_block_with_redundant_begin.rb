do_something do
  begin
  ^^^^^ Redundant `begin` block detected.
    foo
  rescue => e
    bar
  end
end
