def func
  a = nil
  b = [1, 2, 3]

  (a || b).each do |n|
    puts n
  end
end
