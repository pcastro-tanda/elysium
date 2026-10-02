def object.hash
  1.hash ^ 2.hash ^ 3.hash
  ^^^^^^^^^^^^^^^^^^^^^^^^ Use `[...].hash` instead of combining hash values manually.
end
