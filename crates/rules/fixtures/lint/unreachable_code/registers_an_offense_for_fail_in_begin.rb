def something
  array.each do |item|
    begin
      fail
      bar
      ^^^ Unreachable code detected.
    end
  end
end
