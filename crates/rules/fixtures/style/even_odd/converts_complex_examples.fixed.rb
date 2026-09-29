if y.even?
  method == :== ? :even : :odd
elsif x.odd?
  method == :== ? :odd : :even
end
