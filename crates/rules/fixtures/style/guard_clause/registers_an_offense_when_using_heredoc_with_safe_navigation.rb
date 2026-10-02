def func
  if condition
  ^^ Use a guard clause (`raise obj&.do_something(<<~MESSAGE) unless condition`) instead of wrapping the code inside a conditional expression.
    do_something
  else
    raise obj&.do_something(<<~MESSAGE)
      oops
    MESSAGE
  end
end
