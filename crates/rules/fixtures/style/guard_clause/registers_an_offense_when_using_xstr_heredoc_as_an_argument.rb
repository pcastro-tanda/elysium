def func
  unless condition
  ^^^^^^ Use a guard clause (`raise <<~`MESSAGE` unless condition`) instead of wrapping the code inside a conditional expression.
    raise <<~`MESSAGE`
      oops
    MESSAGE
  else
    foo
  end
end
