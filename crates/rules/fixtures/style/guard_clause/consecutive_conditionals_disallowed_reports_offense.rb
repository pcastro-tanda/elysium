def func
  if foo?
    work
  end

  if bar?
  ^^ Use a guard clause (`return unless bar?`) instead of wrapping the code inside a conditional expression.
    work
  end
end
