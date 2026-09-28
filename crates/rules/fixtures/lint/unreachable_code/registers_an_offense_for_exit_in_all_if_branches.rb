def something
  array.each do |item|
    if cond
      exit
    else
      exit
    end
    bar
    ^^^ Unreachable code detected.
  end
end
