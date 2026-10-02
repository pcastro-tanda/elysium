array.select do |x|
^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `select` with a kind check.
  x.is_a?(Foo)
end
