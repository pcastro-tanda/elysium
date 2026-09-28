def something
  array.each do |item|
    if cond
      fail
    else
      fail
    end
    bar
    ^^^ Unreachable code detected.
  end
end
