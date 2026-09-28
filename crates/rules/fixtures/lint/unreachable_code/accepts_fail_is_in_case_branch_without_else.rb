def something
  array.each do |item|
    case cond
    when 1
      something
      fail
    when 2
      something2
      fail
    end
    bar
  end
end
