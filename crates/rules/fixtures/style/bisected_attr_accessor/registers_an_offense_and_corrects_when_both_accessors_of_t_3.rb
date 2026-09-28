class Foo
  attr_reader :baz, :bar, :quux
                    ^^^^ Combine both accessors into `attr_accessor :bar`.
  attr_writer :bar, :zoo
              ^^^^ Combine both accessors into `attr_accessor :bar`.
  other_macro :something
end
