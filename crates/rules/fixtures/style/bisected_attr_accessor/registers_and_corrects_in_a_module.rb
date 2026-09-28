module Foo
  attr_reader :foo
              ^^^^ Combine both accessors into `attr_accessor :foo`.
  attr_writer :foo, :bar
              ^^^^ Combine both accessors into `attr_accessor :foo`.

  private

  attr_reader :bar, :baz
                    ^^^^ Combine both accessors into `attr_accessor :baz`.
  attr_writer :baz
              ^^^^ Combine both accessors into `attr_accessor :baz`.
end
