define_singleton_method(:func) do
  if _1
  ^^ Use a guard clause (`return unless _1`) instead of wrapping the code inside a conditional expression.
    # TODO
  end
end

define_singleton_method(:func) do
  unless _1
  ^^^^^^ Use a guard clause (`return if _1`) instead of wrapping the code inside a conditional expression.
    # TODO
  end
end
