class Foo
  attr_reader :bar

  class << self
    attr_accessor :baz

    private

    attr_reader :quux
  end
end
