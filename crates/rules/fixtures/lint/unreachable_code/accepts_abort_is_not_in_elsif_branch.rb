def something
  array.each do |item|
    if cond
      something
      abort
    elsif cond2
      something2
    else
      something3
      abort
    end
    bar
  end
end
