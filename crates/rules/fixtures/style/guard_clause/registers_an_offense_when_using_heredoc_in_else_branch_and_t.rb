def func
  if condition
  ^^ Use a guard clause (`raise <<~MESSAGE unless condition`) instead of wrapping the code inside a conditional expression.
    x = 1
    do_something(x)
  else
    raise <<~MESSAGE
      oops
    MESSAGE
  end
end
