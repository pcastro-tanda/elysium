def something
  array.each do |item|
    if cond
      something
    else
      something2
      fail
    end
    bar
  end
end
