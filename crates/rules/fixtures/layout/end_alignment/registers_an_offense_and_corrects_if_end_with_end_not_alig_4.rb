var = if test
        foo
      end && ""
      ^^^ `end` at 3, 6 is not aligned with `var = if` at 1, 0.
