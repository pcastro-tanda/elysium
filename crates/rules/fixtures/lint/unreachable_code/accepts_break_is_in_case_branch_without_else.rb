def something
  array.each do |item|
    case cond
    when 1
      something
      break
    when 2
      something2
      break
    end
    bar
  end
end
