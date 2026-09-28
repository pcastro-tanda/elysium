def something
  array.each do |item|
    if cond
      something
      exit
    elsif cond2
      something2
      exit
    else
      something3
      exit
    end
    bar
    ^^^ Unreachable code detected.
  end
end
