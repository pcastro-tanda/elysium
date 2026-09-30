bar = case foo
^^^^^^^^^^^^^^ Assign variables inside of conditionals.
      when foobar
        something
        1
      when baz
        something_other
        2
      else
        something_else
        3
      end
