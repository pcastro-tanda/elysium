def something
  array.each do |item|
    if cond
      something
      break
    else
      something2
    end
    bar
  end
end
