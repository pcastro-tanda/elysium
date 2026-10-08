class Person < Data.define(:first_name, :last_name) do end
               ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Don't extend an instance initialized by `Data.define`. Use a block to customize the class.
end
