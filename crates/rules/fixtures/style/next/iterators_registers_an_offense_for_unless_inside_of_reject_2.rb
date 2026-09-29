[].reject! do |o|
  unless o == 1
  ^^^^^^^^^^^^^ Use `next` to skip iteration.
    true
  end
end
