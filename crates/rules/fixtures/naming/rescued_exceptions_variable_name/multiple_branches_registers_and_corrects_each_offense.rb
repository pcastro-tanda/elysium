begin
  something
rescue MyException => exc
                      ^^^ Use `e` instead of `exc`.
  # do something
rescue OtherException => exc
                         ^^^ Use `e` instead of `exc`.
  # do something else
end
