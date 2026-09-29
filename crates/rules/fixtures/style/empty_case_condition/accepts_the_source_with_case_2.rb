def example
  # Comment before everything
  case :a # first comment
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
