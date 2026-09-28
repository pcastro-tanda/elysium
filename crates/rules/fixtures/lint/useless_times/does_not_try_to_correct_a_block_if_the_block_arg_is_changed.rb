1.times do |i|
^^^^^^^^^^^^^^ Useless call to `1.times` detected.
  do_something(i)
  i += 1
  do_something_else(i)
end
