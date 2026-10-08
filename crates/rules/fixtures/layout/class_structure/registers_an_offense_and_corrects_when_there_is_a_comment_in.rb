class Foo
  # This is a comment for macro method.
  validates :attr
  attr_reader :foo
  ^^^^^^^^^^^^^^^^ `attribute_macros` is supposed to appear before `macros`.
end
