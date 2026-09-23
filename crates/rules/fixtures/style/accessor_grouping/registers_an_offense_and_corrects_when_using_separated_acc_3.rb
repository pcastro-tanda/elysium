class Foo
  attr_reader :bar

  class << self
    attr_reader :baz1, :baz2
    ^^^^^^^^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
    attr_reader :baz3
    ^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.

    private

    attr_reader :quux1
    ^^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
    attr_reader :quux2
    ^^^^^^^^^^^^^^^^^^ Group together all `attr_reader` attributes.
  end
end
