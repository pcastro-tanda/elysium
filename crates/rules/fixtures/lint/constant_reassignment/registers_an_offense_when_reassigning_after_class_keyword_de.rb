class Parent
  class FooError < StandardError; end
  FooError = Class.new(RuntimeError)
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Constant `FooError` is already assigned in this namespace.
end
