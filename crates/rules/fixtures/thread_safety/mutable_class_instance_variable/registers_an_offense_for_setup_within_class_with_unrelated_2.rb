class FooTest < MyTest
  setup do
    @something = { a: "A" }
                 ^^^^^^^^^^ Freeze mutable objects assigned to class instance variables.
  end
end
