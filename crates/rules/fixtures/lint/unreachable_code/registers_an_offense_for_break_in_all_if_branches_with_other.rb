def something
  array.each do |item|
    if cond
      something
      break
    else
      something2
      break
    end
    bar
    ^^^ Unreachable code detected.
  end
end
