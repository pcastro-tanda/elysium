def func
  some_preceding_statements
  case x
  in y
    res = 1
    ^^^^^^^ Redundant assignment before returning detected.
    res
  in z
    2
  in q
  else
    res = 3
    ^^^^^^^ Redundant assignment before returning detected.
    res
  end
end
