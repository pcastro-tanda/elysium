def test(some_arg)
  _first, some_lvar, _third  = do_something
  some_lvar.attr = 5
  ^^^^^^^^^ Useless setter call to local variable `some_lvar`.
end
