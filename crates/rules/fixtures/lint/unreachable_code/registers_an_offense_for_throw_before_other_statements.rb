def something
  array.each do |item|
    throw
    bar
    ^^^ Unreachable code detected.
  end
end
