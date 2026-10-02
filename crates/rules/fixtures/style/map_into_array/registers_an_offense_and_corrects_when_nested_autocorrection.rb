dest = []
src.each do |e|
^^^^^^^^^^^^^^^ Use `map` instead of `each` to map elements into an array.
  dest << (
    dest2 = []
    src.each do |e|
    ^^^^^^^^^^^^^^^ Use `map` instead of `each` to map elements into an array.
      dest2 << e
    end
    dest2
  )
end
