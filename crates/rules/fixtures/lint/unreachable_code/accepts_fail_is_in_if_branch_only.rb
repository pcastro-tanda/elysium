def something
  array.each do |item|
    if cond
      something
      fail
    else
      something2
    end
    bar
  end
end
