def something
  array.each do |item|
    if cond
      something
      break
    elsif cond2
      something2
      break
    else
      something3
      break
    end
    bar
    ^^^ Unreachable code detected.
  end
end
