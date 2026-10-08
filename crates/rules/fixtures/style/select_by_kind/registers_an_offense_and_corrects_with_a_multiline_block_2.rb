array.find_all do |x|
^^^^^^^^^^^^^^^^^^^^^ Prefer `grep` to `find_all` with a kind check.
  x.is_a?(Foo)
end
