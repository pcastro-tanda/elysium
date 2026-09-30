bar %= case foo
^^^^^^^^^^^^^^^ Assign variables inside of conditionals.
              when "a"
                something
                1
              else
                something_else
                2
              end
