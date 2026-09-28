1.times do |i|
^^^^^^^^^^^^^^ Useless call to `1.times` detected.
  do_something(i)
  i, j = i * 2, i * 3
  do_something_else(i)
end
