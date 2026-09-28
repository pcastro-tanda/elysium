class Foo
  attr_writer :foo
              ^^^^ Combine both accessors into `attr_accessor :foo`.
  attr_reader :foo
              ^^^^ Combine both accessors into `attr_accessor :foo`.
  other_macro :something
end
