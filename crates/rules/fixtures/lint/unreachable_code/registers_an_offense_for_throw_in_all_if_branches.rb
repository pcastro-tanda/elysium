def something
  array.each do |item|
    if cond
      throw
    else
      throw
    end
    bar
    ^^^ Unreachable code detected.
  end
end
