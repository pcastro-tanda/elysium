def something
  array.each do |item|
    begin
      exit
      bar
      ^^^ Unreachable code detected.
    end
  end
end
