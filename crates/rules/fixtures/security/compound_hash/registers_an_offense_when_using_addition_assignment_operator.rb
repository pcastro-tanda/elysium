def hash
  h = 0

  things.each do |thing|
    h += thing.hash
    ^^^^^^^^^^^^^^^ Use `[...].hash` instead of combining hash values manually.
  end

  h
end
