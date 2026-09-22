def func
  if something
  ^^ Use a guard clause (`test and return unless something`) instead of wrapping the code inside a conditional expression.
    work
  else
    test and return
  end
end
