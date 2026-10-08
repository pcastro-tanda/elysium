dest = src.map do |e|
  if cond?
    e
  else
    e * 2
  end
end
