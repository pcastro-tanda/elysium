def method(x, &block)
  do_something if block_given?
                  ^^^^^^^^^^^^ Check block argument explicitly instead of using `block_given?`.
end
