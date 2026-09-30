define_method :method_name do
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Cyclomatic complexity for `method_name` is too high. [3/1]
  (_1.._2).map do |i|                          # map: +1
    i * 2
  end.each.with_index { |val, i| puts val, i } # each: +0, with_index: +1
  return treasure.map
end
