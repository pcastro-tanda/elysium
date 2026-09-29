[].reject do |o|
  if o == 1
  ^^^^^^^^^ Use `next` to skip iteration.
    true
  end
end
