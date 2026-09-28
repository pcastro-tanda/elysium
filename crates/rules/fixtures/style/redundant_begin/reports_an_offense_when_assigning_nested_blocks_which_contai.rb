var = do_something do
  begin
  ^^^^^ Redundant `begin` block detected.
    do_something do
      begin
      ^^^^^ Redundant `begin` block detected.
        foo
      ensure
        bar
      end
    end
  ensure
    baz
  end
end
