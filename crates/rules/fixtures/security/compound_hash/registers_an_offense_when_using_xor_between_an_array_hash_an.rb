def hash
  [red, blue, green, alpha].hash ^ self.class.hash
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `[...].hash` instead of combining hash values manually.
end
