def method(x, &)
  raise ArgumentError, "block required" unless block_given?
  do_something(&)
end
