e = 'error message'
begin
  something
rescue StandardError => e1
  log(e, e1)
end
