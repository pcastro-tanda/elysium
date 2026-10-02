array.select do |x|
  next if x.even?
  x.match? /regexp/
end
