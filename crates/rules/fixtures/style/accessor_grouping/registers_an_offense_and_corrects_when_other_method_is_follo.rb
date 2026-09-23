class Foo
  other_macro :zoo, :woo

  attr_reader :foo, :bar
  ^^^^^^^^^^^^^^^^^^^^^^ Use one attribute per `attr_reader`.
end
