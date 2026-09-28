def something
  array.each do |item|
    if cond
      something
      exit
    else
      something2
      exit
    end
    bar
    ^^^ Unreachable code detected.
  end
end
