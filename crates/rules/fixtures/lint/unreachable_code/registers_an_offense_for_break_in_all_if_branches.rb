def something
  array.each do |item|
    if cond
      break
    else
      break
    end
    bar
    ^^^ Unreachable code detected.
  end
end
