def something
  array.each do |item|
    if cond
      something
      exit
    else
      something2
    end
    bar
  end
end
