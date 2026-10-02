values.inject(nil) do |result, value|
  next value if something?
  result
end
