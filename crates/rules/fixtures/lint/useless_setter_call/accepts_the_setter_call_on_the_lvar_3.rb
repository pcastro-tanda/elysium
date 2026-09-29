def test(some_arg)
  some_lvar = nil
  some_lvar ||= some_arg
  some_lvar.attr = 5
end
