def method
  begin # comment 1
  ^^^^^ Redundant `begin` block detected.
    do_some_stuff
  rescue # comment 2
  end # comment 3
end
