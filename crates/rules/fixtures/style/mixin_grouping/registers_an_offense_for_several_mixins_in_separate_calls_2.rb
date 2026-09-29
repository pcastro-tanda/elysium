class Foo
  include Bar, Baz
  ^^^^^^^^^^^^^^^^ Put `include` mixins in a single statement.
  include FooBar, FooBaz
  ^^^^^^^^^^^^^^^^^^^^^^ Put `include` mixins in a single statement.
  include Qux, FooBarBaz
  ^^^^^^^^^^^^^^^^^^^^^^ Put `include` mixins in a single statement.
end
