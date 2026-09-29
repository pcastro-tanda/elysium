def example
  # Comment before everything
  case # first comment
  ^^^^ Do not use empty `case` condition, instead use an `if` expression.
  # condition a
  # This is a multi-line comment
  when 1 == 2
    foo
  # condition b
  when 1 == 1
    bar
  # condition c
  else
    baz
  end
end
