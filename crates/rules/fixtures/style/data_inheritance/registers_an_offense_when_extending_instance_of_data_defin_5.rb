class Person < Data.define(:first_name, :last_name) { def age = 42 }
               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Don't extend an instance initialized by `Data.define`. Use a block to customize the class.
end
