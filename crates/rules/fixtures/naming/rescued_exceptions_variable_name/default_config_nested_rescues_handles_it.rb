begin
rescue StandardError => e1
                        ^^ Use `e` instead of `e1`.
  begin
    log(e1)
  rescue StandardError => e2
    log(e1, e2)
  end
end
