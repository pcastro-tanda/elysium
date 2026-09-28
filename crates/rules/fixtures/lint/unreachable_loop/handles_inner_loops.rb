until x > 0
^^^^^^^^^^^ This loop will have at most one iteration.

  items.each do |item|
    next if item.odd?
    break
  end

  if x > 0
    break some_value
  else
    raise MyError
  end

  loop do
  ^^^^^^^ This loop will have at most one iteration.

    case y
    when 1
      return something
    when 2
      break
    else
      throw :exit
    end
  end
end
