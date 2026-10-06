foo do
  x.map { |a| a[:foo] }
    ^^^^^^^^^^^^^^^^^^^ Prefer `pluck(:foo)` over `map { |a| a[:foo] }`.
end
