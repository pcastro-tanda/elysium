def func
  if condition
  ^^ Use a guard clause (`raise do_something(<<~MESSAGE) unless condition`) instead of wrapping the code inside a conditional expression.
    foo
  else
    raise do_something(<<~MESSAGE)
      text
    MESSAGE
  end
end
