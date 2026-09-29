def test
  some_arg = {}
  some_arg[:attr] = 1
  ^^^^^^^^ Useless setter call to local variable `some_arg`.
end
