def something
  array.each do |item|
    exit!
    bar
    ^^^ Unreachable code detected.
  end
end
