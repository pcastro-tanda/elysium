[].each_with_object({}) do |o, a|
  unless o == 1
  ^^^^^^^^^^^^^ Use `next` to skip iteration.
    a[o] = {}
  end
end
