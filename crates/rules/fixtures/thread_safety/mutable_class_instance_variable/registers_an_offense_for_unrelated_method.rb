class FooTest < MyTest
  set_up do
    @something = { a: "A" }
                 ^^^^^^^^^^ Freeze mutable objects assigned to class instance variables.
  end
end
