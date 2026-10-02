class Foo
  attr_accessor :foo
  CONST = 'wrong place'
  ^^^^^^^^^^^^^^^^^^^^^ `all_constants` is supposed to appear before `attribute_macros`.
end
