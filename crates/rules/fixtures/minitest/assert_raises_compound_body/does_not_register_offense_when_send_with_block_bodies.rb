assert_raises(MyError) do
  foo do
    bar
    baz
  end
end
