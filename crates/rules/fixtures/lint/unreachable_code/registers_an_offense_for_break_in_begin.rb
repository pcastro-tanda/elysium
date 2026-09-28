def something
  array.each do |item|
    begin
      break
      bar
      ^^^ Unreachable code detected.
    end
  end
end
