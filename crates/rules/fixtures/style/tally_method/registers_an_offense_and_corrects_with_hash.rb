array.each_with_object(::Hash.new(0)) { |x, h| h[x] += 1 }
      ^^^^^^^^^^^^^^^^ Use `tally` instead of `each_with_object`.
