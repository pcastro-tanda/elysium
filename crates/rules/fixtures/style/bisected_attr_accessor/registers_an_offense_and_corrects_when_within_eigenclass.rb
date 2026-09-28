class Foo
  attr_reader :bar

  class << self
    attr_reader :baz
                ^^^^ Combine both accessors into `attr_accessor :baz`.
    attr_writer :baz
                ^^^^ Combine both accessors into `attr_accessor :baz`.

    private

    attr_reader :quux
  end
end
