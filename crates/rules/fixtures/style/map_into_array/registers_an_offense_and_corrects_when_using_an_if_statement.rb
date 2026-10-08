dest = []
src.each do |e|
^^^^^^^^^^^^^^^ Use `map` instead of `each` to map elements into an array.
  dest << if cond?
    e
  else
    e * 2
  end
end
