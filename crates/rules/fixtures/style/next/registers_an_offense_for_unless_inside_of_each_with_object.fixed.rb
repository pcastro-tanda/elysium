[].each_with_object({}) do |o, a|
  next if o == 1
  a[o] = {}
end
