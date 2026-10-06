def method(a, &block)
  block ||= -> {}
  do_something if block_given?
end
