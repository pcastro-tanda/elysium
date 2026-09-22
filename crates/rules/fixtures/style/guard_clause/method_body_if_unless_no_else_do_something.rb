def func
  if something
  ^^ Use a guard clause (`return unless something`) instead of wrapping the code inside a conditional expression.
    do_something(foo)
  end
end

def func
  unless something
  ^^^^^^ Use a guard clause (`return if something`) instead of wrapping the code inside a conditional expression.
    do_something(foo)
  end
end
