var = Object
begin
  something
rescue => var::StandardError
       ^^ `var::StandardError` is overwritten by `rescue =>`.
end
