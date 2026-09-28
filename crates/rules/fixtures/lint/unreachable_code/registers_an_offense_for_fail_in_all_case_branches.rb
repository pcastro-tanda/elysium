def something
  array.each do |item|
    case cond
    when 1
      something
      fail
    when 2
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
