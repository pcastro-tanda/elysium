def test(some_arg)
  @some_ivar = some_arg
  @some_ivar.do_something
  some_lvar = @some_ivar
  some_lvar.do_something
  some_lvar.attr = 5
end
