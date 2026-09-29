begin
  fail
  begin
    fail
  rescue
    raise
  end
rescue Exception
  #do nothing
end
