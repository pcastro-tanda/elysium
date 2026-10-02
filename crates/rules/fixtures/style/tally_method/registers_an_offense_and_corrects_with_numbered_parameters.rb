array.each_with_object(Hash.new(0)) { _2[_1] += 1 }
      ^^^^^^^^^^^^^^^^ Use `tally` instead of `each_with_object`.
