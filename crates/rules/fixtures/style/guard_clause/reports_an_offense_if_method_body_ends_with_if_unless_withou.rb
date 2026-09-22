def func
  test
  if something
  ^^ Use a guard clause (`return unless something`) instead of wrapping the code inside a conditional expression.
    work
  end
end

def func
  test
  unless something
  ^^^^^^ Use a guard clause (`return if something`) instead of wrapping the code inside a conditional expression.
    work
  end
end
