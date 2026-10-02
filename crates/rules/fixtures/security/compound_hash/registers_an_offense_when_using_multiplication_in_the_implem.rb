def hash
  to_s.hash * -1
  ^^^^^^^^^^^^^^ Use `[...].hash` instead of combining hash values manually.
end
