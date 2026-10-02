foo.map do |item|
  if item.bar?
    1
  else
    2
  end
end.compact
