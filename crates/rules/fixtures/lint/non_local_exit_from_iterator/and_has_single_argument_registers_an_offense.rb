items.each do |item|
  return if item.stock == 0
  ^^^^^^ Non-local exit from iterator, without return value. `next`, `break`, `Array#find`, `Array#any?`, etc. is preferred.
  item.update!(foobar: true)
end
