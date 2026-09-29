begin
  something
rescue MyException => _exc
                      ^^^^ Use `_e` instead of `_exc`.
  # do something
end
