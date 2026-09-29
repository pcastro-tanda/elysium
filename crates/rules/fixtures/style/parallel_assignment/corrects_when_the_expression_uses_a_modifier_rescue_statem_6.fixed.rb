def foo
  begin
    a = '1'
    b = '2'
  rescue
    foo
  end
  something_else
end
