array.find_all do |x|
  next if x.even?
  x.match? /regexp/
end
