foo = if condition
      else
        foo
        ^^^ Remove the self-assignment branch.
      end
