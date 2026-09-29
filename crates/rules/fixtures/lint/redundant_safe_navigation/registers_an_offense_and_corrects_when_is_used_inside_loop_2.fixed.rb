until foo.respond_to?(:bar)
  do_something
end

begin
  do_something
end until foo.respond_to?(:bar)
