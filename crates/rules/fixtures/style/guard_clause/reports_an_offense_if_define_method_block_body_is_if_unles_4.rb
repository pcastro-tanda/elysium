define_method(:func) do
  if it
  ^^ Use a guard clause (`return unless it`) instead of wrapping the code inside a conditional expression.
    # TODO
  end
end

define_method(:func) do
  unless it
  ^^^^^^ Use a guard clause (`return if it`) instead of wrapping the code inside a conditional expression.
    # TODO
  end
end
