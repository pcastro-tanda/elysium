def something
  array.each do |item|
    begin
      throw
      bar
      ^^^ Unreachable code detected.
    end
  end
end
