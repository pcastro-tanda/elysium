retried = false

begin
  do_something
rescue
  fail if retried
  retried = true
  retry
end
