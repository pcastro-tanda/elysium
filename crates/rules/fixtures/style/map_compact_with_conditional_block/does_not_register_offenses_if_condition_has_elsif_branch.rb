foo.map do |item|
  if item.bar?
    item
  elsif
    baz
  else
    next
  end
end.compact
