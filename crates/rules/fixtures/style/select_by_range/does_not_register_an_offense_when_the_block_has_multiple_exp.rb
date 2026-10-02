array.select do |x|
  next if x.even?
  x.between?(1, 10)
end
