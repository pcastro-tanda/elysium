def func
  if condition
  ^^ Use a guard clause (`raise <<~MESSAGE.strip unless condition`) instead of wrapping the code inside a conditional expression.
    foo
  else
    raise <<~MESSAGE.strip
      oops
    MESSAGE
  end
end
