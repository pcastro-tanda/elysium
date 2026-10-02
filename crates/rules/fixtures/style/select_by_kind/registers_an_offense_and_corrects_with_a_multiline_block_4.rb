array.reject do |x|
^^^^^^^^^^^^^^^^^^^ Prefer `grep_v` to `reject` with a kind check.
  x.is_a?(Foo)
end
