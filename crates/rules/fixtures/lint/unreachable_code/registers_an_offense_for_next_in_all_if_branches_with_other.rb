def something
  array.each do |item|
    if cond
      something
      next
    else
      something2
      next
    end
    bar
    ^^^ Unreachable code detected.
  end
end
