def hash
  foo.hash ^ bar.hash << 1 ^ biz.hash << 2 ^ bar.hash << 3
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `[...].hash` instead of combining hash values manually.
end
