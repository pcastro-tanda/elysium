def test(some_arg)
  some_arg = Top.new
  some_arg.attr = 5
  ^^^^^^^^ Useless setter call to local variable `some_arg`.
end
