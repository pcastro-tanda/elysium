foo.map do |item|
  if item.bar?
    item
  else
    next
  end
end.compact(arg)
