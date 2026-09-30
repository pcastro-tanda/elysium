retry_count = 0

begin
  do_something
rescue
  fail if (retry_count += 1) > 3
  retry
end
