array.any? do |x|
^^^^^^^^^^^^^^^^^ Prefer `any?(Integer)` to `any? { ... }` with a kind check.
  x.is_a?(Integer)
end
