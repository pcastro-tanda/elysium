begin
  status = :initial
  connect_sometimes_fails!
  status = :connected
  fetch_sometimes_fails!
  status = :fetched
rescue
  puts status
end
