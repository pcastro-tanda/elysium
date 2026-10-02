array.filter do |x|
^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `filter` with a kind check.
  x.is_a?(Foo)
end
