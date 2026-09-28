def my_method
  0.times do
  ^^^^^^^^^^ Useless call to `0.times` detected.
    do_something
    do_something_else
  end
end
