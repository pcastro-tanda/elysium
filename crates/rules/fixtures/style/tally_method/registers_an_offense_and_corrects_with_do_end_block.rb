array.each_with_object(Hash.new(0)) do |item, counts|
      ^^^^^^^^^^^^^^^^ Use `tally` instead of `each_with_object`.
  counts[item] += 1
end
