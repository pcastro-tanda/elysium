def func
  1
  2
  return 3
  ^^^^^^ Redundant `return` detected.
rescue SomeException
  4
  return 5
  ^^^^^^ Redundant `return` detected.
rescue AnotherException
  return 6
  ^^^^^^ Redundant `return` detected.
ensure
  return 7
end
