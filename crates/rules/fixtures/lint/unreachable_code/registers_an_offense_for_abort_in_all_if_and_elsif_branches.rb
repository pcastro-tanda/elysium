def something
  array.each do |item|
    if cond
      something
      abort
    elsif cond2
      something2
      abort
    else
      something3
      abort
    end
    bar
    ^^^ Unreachable code detected.
  end
end
