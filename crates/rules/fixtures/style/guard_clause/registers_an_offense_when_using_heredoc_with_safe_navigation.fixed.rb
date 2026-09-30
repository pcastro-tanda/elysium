def func
  raise obj&.do_something(<<~MESSAGE) unless condition
      oops
    MESSAGE
do_something
end
