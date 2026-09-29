def test(some_arg)
  some_lvar = some_arg
  some_lvar += some_arg
  some_lvar.attr = 5
  ^^^^^^^^^ Useless setter call to local variable `some_lvar`.
end
