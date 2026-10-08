[].tap do |dest|
  src.each { |e| dest << e * 2 }
  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Use `map` instead of `each` to map elements into an array.
end
