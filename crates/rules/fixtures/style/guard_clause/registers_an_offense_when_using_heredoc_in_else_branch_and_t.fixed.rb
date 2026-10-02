def func
  raise <<~MESSAGE unless condition
      oops
    MESSAGE
x = 1
    do_something(x)
end
