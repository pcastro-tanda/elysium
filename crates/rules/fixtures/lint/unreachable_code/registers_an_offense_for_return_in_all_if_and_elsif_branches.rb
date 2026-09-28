def something
  array.each do |item|
    if cond
      something
      return
    elsif cond2
      something2
      return
    else
      something3
      return
    end
    bar
    ^^^ Unreachable code detected.
  end
end
