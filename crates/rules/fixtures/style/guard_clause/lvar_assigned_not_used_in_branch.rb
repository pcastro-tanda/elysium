def func
  if (foo = bar)
  ^^ Use a guard clause (`return baz if (foo = bar)`) instead of wrapping the code inside a conditional expression.
    return baz
  else
    qux
  end
end
