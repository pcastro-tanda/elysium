[].each_with_object({}) do |o, a|
  if o == 1
  ^^^^^^^^^ Use `next` to skip iteration.
    a[o] = {}
  end
end
