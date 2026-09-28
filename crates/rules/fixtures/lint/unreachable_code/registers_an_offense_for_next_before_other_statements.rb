def something
  array.each do |item|
    next
    bar
    ^^^ Unreachable code detected.
  end
end
