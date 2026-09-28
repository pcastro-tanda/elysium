def something
  array.each do |item|
    if cond
      abort
    else
      abort
    end
    bar
    ^^^ Unreachable code detected.
  end
end
