begin
  something
rescue MyException => exc
                      ^^^ Use `e` instead of `exc`.
  # do something
end
