define_singleton_method(:foo) do
  return something
  ^^^^^^ Redundant `return` detected.
end
