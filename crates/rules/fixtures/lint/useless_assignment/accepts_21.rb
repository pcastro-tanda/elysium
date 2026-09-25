begin
  status = :initial
  connect_sometimes_fails!
  status = :connected
  fetch_sometimes_fails!
  status = :fetched
ensure
  puts status
end
