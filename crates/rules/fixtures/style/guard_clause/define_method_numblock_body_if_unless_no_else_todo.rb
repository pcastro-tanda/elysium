define_method(:func) do
  if something
  ^^ Use a guard clause (`return unless something`) instead of wrapping the code inside a conditional expression.
    # TODO
  end
end

define_method(:func) do
  unless something
  ^^^^^^ Use a guard clause (`return if something`) instead of wrapping the code inside a conditional expression.
    # TODO
  end
end
