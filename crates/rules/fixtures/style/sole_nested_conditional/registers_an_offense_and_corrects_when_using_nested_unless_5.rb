if foo
  unless bar
  ^^^^^^ Consider merging nested conditions into outer `if` conditions.
    if baz
    ^^ Consider merging nested conditions into outer `unless` conditions.
      do_something
    end
  end
end
