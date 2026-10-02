class Foo
  validates :name
  CONST = do_something.freeze
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^ `constants` is supposed to appear before `macros`.
  attr_reader :foo
  ^^^^^^^^^^^^^^^^ `attribute_macros` is supposed to appear before `macros`.
end
