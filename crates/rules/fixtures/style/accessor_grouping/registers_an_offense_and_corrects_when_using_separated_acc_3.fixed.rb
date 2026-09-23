class Foo
  attr_reader :bar

  class << self
    attr_reader :baz1, :baz2, :baz3

    private

    attr_reader :quux1, :quux2
  end
end
