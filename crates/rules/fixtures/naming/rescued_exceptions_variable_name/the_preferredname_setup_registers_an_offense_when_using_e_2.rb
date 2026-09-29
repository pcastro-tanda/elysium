begin
  something
rescue MyException => _e
                      ^^ Use `_exception` instead of `_e`.
  # do something
end
