def func
  some_preceding_statements
  case x
  in y then return 1
            ^^^^^^ Redundant `return` detected.
  in z then return 2
            ^^^^^^ Redundant `return` detected.
  in q
  else
    return 3
    ^^^^^^ Redundant `return` detected.
  end
end
