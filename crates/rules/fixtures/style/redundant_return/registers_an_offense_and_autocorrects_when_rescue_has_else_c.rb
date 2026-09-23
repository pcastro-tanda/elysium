def func
  return 3
rescue SomeException
else
  return 4
  ^^^^^^ Redundant `return` detected.
end
