foo&.select! do |e|
^^^^^^^^^^^^^^^^^^^ Use `reject!` instead of inverting `select!`.
  something
  e&.bar&.!
end
