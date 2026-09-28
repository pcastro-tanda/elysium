def something
  array.each do |item|
    case cond
    when 1
      something
      break
    when 2
      something2
      break
    else
      something3
      break
    end
    bar
    ^^^ Unreachable code detected.
  end
end
