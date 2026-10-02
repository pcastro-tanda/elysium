class A::FooError < StandardError; end
A::FooError = Class.new(RuntimeError)
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Constant `A::FooError` is already assigned in this namespace.
