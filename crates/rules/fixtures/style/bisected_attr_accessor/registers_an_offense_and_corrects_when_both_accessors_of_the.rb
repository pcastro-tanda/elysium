class Foo
  attr_reader :bar
              ^^^^ Combine both accessors into `attr_accessor :bar`.
  attr_writer :bar
              ^^^^ Combine both accessors into `attr_accessor :bar`.
  other_macro :something
end
