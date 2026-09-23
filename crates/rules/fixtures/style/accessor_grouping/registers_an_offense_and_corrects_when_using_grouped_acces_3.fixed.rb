class Foo
  attr_reader :bar

  class << self
    attr_reader :baz1
    attr_reader :baz2
    attr_reader :baz3

    private

    attr_reader :quux1
    attr_reader :quux2
  end
end
