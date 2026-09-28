def something
  array.each do |item|
    if cond
      something
      abort
    else
      something2
      abort
    end
    bar
    ^^^ Unreachable code detected.
  end
end
