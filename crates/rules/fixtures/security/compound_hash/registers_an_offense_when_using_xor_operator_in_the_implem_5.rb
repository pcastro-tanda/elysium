define_singleton_method(:hash) do
  1.hash ^ 2.hash ^ 3.hash
  ^^^^^^^^^^^^^^^^^^^^^^^^ Use `[...].hash` instead of combining hash values manually.
end
