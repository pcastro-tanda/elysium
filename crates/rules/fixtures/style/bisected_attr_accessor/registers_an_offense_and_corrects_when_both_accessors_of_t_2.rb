class Foo
  ATTRIBUTES = %i[foo bar]
  attr_reader *ATTRIBUTES
              ^^^^^^^^^^^ Combine both accessors into `attr_accessor *ATTRIBUTES`.
  attr_writer *ATTRIBUTES
              ^^^^^^^^^^^ Combine both accessors into `attr_accessor *ATTRIBUTES`.
  other_macro :something
end
