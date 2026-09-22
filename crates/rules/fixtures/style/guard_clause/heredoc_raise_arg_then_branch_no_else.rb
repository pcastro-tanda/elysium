def func
  if condition
  ^^ Use a guard clause (`return unless condition`) instead of wrapping the code inside a conditional expression.
    raise <<~MESSAGE
      oops
    MESSAGE
  end
end
