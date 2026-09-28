def something
  array.each do |item|
    case cond
    when 1
      something
      raise
    when 2
      something2
      raise
    end
    bar
  end
end
