def something
  array.each do |item|
    if cond
      something
    else
      something2
      abort
    end
    bar
  end
end
