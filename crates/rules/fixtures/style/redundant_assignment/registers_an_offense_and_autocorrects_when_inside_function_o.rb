def func
  1
  x = 2
  ^^^^^ Redundant assignment before returning detected.
  x
rescue SomeException
  3
  x = 4
  ^^^^^ Redundant assignment before returning detected.
  x
rescue AnotherException
  5
end
