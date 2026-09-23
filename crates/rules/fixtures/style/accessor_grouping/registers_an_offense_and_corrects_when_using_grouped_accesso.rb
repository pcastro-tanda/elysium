class Foo
  attr_reader :bar, :baz
  ^^^^^^^^^^^^^^^^^^^^^^ Use one attribute per `attr_reader`.
  attr_accessor :quux
  other_macro :zoo, :woo
end
