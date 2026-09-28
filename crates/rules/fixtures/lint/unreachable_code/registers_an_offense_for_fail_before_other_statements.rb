def something
  array.each do |item|
    fail
    bar
    ^^^ Unreachable code detected.
  end
end
