foo.bar = case foo
^^^^^^^^^^^^^^^^^^ Assign variables inside of conditionals.
              when "a"
                1
              when "b"
                2
              else
                3
              end
