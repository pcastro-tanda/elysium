def func
  some_preceding_statements
  if x
    z = 1
    ^^^^^ Redundant assignment before returning detected.
    z
  elsif y
    2
  else
    z = 3
    ^^^^^ Redundant assignment before returning detected.
    z
  end
end
