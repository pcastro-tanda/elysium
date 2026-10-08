begin
  something
rescue => StandardError
       ^^ `StandardError` is overwritten by `rescue =>`.
end
