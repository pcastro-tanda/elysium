def my_method
  1.times do
  ^^^^^^^^^^ Useless call to `1.times` detected.
    do_something
    do_something_else
  end
end
