def something
  array.each do |item|
    if cond
      next
    else
      next
    end
    bar
    ^^^ Unreachable code detected.
  end
end
