class Foo
  attr_reader :bar
              ^^^^ Combine both accessors into `attr_accessor :bar`.
  attr_writer :bar
              ^^^^ Combine both accessors into `attr_accessor :bar`.

  private

  attr_writer :baz
              ^^^^ Combine both accessors into `attr_accessor :baz`.
  attr_reader :baz
              ^^^^ Combine both accessors into `attr_accessor :baz`.
end
