def func
  some_preceding_statements
  if x
    return 1
    ^^^^^^ Redundant `return` detected.
  elsif y
    return 2
    ^^^^^^ Redundant `return` detected.
  else
    return 3
    ^^^^^^ Redundant `return` detected.
  end
end
