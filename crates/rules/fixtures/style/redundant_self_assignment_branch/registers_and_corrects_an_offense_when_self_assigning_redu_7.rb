foo = if condition
        foo
        ^^^ Remove the self-assignment branch.
      else
        bar
      end
