def something
  array.each do |item|
    case cond
    when 1
      something
      throw
    when 2
      something2
      throw
    end
    bar
  end
end
