begin
rescue StandardError => e
  begin
    log(e)
  rescue StandardError => e2
    log(e, e2)
  end
end
