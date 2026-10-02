foo = if condition
        bar
      else
        foo
        ^^^ Remove the self-assignment branch.
      end
