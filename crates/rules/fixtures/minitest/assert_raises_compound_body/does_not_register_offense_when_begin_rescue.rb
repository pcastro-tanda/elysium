assert_raises(MyError) do
  begin
    foo
    bar
  rescue
    baz
  end
end
