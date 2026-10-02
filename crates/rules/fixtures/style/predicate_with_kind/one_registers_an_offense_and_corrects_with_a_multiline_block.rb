array.one? do |x|
^^^^^^^^^^^^^^^^^ Prefer `one?(Integer)` to `one? { ... }` with a kind check.
  x.is_a?(Integer)
end
