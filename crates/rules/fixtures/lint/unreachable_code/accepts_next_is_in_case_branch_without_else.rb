def something
  array.each do |item|
    case cond
    when 1
      something
      next
    when 2
      something2
      next
    end
    bar
  end
end
