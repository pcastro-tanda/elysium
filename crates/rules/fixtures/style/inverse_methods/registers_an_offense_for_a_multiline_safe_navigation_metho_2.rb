foo&.reject do |e|
^^^^^^^^^^^^^^^^^^ Use `select` instead of inverting `reject`.
  something
  e&.bar&.!
end
