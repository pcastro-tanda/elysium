def something
  array.each do |item|
    if cond
      something
      fail
    else
      something2
      fail
    end
    bar
    ^^^ Unreachable code detected.
  end
end
