def something
  array.each do |item|
    if cond
      something
      fail
    elsif cond2
      something2
      fail
    else
      something3
      fail
    end
    bar
    ^^^ Unreachable code detected.
  end
end
