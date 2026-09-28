def something
  array.each do |item|
    begin
      return
      bar
      ^^^ Unreachable code detected.
    end
  end
end
