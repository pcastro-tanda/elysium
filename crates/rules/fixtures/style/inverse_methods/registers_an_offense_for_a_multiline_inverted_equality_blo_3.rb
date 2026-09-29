foo.select! do |e|
^^^^^^^^^^^^^^^^^^ Use `reject!` instead of inverting `select!`.
  something
  something_else
  e != 2
end
