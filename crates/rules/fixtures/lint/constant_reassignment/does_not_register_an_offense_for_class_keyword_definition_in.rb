if condition
  class FooError < StandardError; end
end
FooError = Class.new(StandardError)
