foo do
  x.collect { |a| a[:foo] }
    ^^^^^^^^^^^^^^^^^^^^^^^ Prefer `pluck(:foo)` over `collect { |a| a[:foo] }`.
end
