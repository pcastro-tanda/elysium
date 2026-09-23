def func
  some_preceding_statements
  case x
  when y then return 1
              ^^^^^^ Redundant `return` detected.
  when z then return 2
              ^^^^^^ Redundant `return` detected.
  when q
  else
    return 3
    ^^^^^^ Redundant `return` detected.
  end
end
