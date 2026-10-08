array.none? do |x|
  next if x.nil?
  x.is_a?(Integer)
end
