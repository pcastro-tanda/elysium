def something
  array.each do |item|
    if cond
      something
      abort
    end
    bar
  end
end
