class Foo
  attr_reader :foo, :bar, :baz
                          ^^^^ Combine both accessors into `attr_accessor :baz`.
                    ^^^^ Combine both accessors into `attr_accessor :bar`.
              ^^^^ Combine both accessors into `attr_accessor :foo`.
  attr_writer :foo, :bar, :baz
                          ^^^^ Combine both accessors into `attr_accessor :baz`.
                    ^^^^ Combine both accessors into `attr_accessor :bar`.
              ^^^^ Combine both accessors into `attr_accessor :foo`.
end
