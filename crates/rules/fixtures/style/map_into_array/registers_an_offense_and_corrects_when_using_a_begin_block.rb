dest = []
src.each do |e|
^^^^^^^^^^^^^^^ Use `map` instead of `each` to map elements into an array.
  dest << begin
    foo
    e * 2
  end
end
