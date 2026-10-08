array.filter do |x|
  next if x.even?
  x.match? /regexp/
end
