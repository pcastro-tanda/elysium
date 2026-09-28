class Foo
  ATTRIBUTES = %i[foo bar]
  attr_accessor *ATTRIBUTES
  other_macro :something
end
