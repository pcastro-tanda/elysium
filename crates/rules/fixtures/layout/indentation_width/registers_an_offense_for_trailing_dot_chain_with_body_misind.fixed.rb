Foo.for_operation(arg).
  pluck(:a, :b).
  map do |a, b|
    [b, a]
end
