def something
  array.each do |item|
    if cond
      something
      next
    elsif cond2
      something2
      next
    else
      something3
      next
    end
    bar
    ^^^ Unreachable code detected.
  end
end
