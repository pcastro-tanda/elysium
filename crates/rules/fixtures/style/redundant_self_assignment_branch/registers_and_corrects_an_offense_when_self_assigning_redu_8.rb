foo = if condition
        foo
        ^^^ Remove the self-assignment branch.
      else
        <<~TEXT
          bar
        TEXT
      end
