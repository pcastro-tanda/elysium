array&.each_with_object(Hash.new(0)) { |item, counts| counts[item] += 1 }
       ^^^^^^^^^^^^^^^^ Use `tally` instead of `each_with_object`.
