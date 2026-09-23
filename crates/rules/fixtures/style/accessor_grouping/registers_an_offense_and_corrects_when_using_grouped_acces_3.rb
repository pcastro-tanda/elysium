class Foo
  attr_reader :bar

  class << self
    attr_reader :baz1, :baz2
    ^^^^^^^^^^^^^^^^^^^^^^^^ Use one attribute per `attr_reader`.
    attr_reader :baz3

    private

    attr_reader :quux1, :quux2
    ^^^^^^^^^^^^^^^^^^^^^^^^^^ Use one attribute per `attr_reader`.
  end
end
