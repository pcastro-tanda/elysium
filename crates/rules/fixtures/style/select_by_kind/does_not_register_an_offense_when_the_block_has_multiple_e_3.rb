array.filter do |x|
  next if x.nil?
  x.is_a?(Foo)
end
