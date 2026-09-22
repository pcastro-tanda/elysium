def func
  if something
  ^^ Use a guard clause (`test || raise('message') unless something`) instead of wrapping the code inside a conditional expression.
    work
  else
    test || raise('message')
  end
end
