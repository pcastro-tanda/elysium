dest = []
src.each do |e|
^^^^^^^^^^^^^^^ Use `map` instead of `each` to map elements into an array.
  dest << case foo
  when 1
    e
  else
    e * 2
  end
end
