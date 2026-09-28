def something
  array.each do |item|
    if cond
      something
      throw
    elsif cond2
      something2
      throw
    else
      something3
      throw
    end
    bar
    ^^^ Unreachable code detected.
  end
end
