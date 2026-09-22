def func
  if something
  ^^ Use a guard clause (`work || raise('message') if something`) instead of wrapping the code inside a conditional expression.
    work || raise('message')
  else
    test
  end
end
