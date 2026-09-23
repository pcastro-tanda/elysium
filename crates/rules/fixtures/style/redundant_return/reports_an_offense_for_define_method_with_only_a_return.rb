define_method(:foo) do
  return something
  ^^^^^^ Redundant `return` detected.
end
