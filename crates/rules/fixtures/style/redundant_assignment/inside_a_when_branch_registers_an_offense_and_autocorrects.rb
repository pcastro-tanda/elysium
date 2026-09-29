def func
  some_preceding_statements
  case x
  when y
    res = 1
    ^^^^^^^ Redundant assignment before returning detected.
    res
  when z
    2
  when q
  else
    res = 3
    ^^^^^^^ Redundant assignment before returning detected.
    res
  end
end
