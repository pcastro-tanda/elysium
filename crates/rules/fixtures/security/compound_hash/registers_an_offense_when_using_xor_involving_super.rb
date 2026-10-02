def hash
  foo.hash ^ super ^ bar.hash
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `[...].hash` instead of combining hash values manually.
end
