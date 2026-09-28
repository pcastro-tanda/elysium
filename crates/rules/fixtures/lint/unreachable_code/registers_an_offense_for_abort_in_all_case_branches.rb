def something
  array.each do |item|
    case cond
    when 1
      something
      abort
    when 2
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
