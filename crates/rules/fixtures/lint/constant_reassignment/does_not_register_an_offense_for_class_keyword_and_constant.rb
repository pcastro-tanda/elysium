module A
  class FooError < StandardError; end
end
module B
  FooError = Class.new(StandardError)
end
