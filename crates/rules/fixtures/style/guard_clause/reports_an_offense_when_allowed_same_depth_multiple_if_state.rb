def func
  if foo?
    work
  end

  do_something

  if bar?
  ^^ Use a guard clause (`return unless bar?`) instead of wrapping the code inside a conditional expression.
    work
  end
end
