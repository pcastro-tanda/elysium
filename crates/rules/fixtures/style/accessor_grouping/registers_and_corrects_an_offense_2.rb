class Foo
  attr_reader :a, # comment a
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use one attribute per `attr_reader`.
    :b, # comment b
    :c # comment c
end
