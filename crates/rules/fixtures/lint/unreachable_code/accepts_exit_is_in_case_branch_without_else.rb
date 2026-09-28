def something
  array.each do |item|
    case cond
    when 1
      something
      exit
    when 2
      something2
      exit
    end
    bar
  end
end
