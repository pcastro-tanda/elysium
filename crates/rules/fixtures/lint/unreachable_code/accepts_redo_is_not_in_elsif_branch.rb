def something
  array.each do |item|
    if cond
      something
      redo
    elsif cond2
      something2
    else
      something3
      redo
    end
    bar
  end
end
