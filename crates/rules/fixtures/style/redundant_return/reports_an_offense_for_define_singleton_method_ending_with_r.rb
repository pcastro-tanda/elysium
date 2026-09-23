define_singleton_method(:foo) do
  some_preceding_statements
  return something
  ^^^^^^ Redundant `return` detected.
end
