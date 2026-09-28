def something
  array.each do |item|
    if cond
      something
      fail
    end
    bar
  end
end
