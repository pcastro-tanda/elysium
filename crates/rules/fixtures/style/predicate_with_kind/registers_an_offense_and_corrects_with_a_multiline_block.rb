array.none? do |x|
^^^^^^^^^^^^^^^^^^ Prefer `none?(Integer)` to `none? { ... }` with a kind check.
  x.is_a?(Integer)
end
