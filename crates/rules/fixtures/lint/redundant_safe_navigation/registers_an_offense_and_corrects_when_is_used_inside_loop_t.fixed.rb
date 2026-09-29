while foo.respond_to?(:bar)
  do_something
end

begin
  do_something
end while foo.respond_to?(:bar)
