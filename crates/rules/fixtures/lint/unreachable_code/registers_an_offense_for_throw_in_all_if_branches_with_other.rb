def something
  array.each do |item|
    if cond
      something
      throw
    else
      something2
      throw
    end
    bar
    ^^^ Unreachable code detected.
  end
end
