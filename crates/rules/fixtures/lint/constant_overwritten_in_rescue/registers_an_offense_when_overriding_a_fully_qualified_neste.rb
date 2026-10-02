begin
  something
rescue => ::MyNamespace::MyException
       ^^ `::MyNamespace::MyException` is overwritten by `rescue =>`.
end
