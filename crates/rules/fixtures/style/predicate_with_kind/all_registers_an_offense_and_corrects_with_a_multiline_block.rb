array.all? do |x|
^^^^^^^^^^^^^^^^^ Prefer `all?(Integer)` to `all? { ... }` with a kind check.
  x.is_a?(Integer)
end
