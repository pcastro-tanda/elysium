array.each do |item|
  if
  ^^ Use `next` to skip iteration.
     condition
    next if item.zero?
    do_something
  end
end
