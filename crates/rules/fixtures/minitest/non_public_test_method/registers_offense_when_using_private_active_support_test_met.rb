class FooTest
  private
  test "does something"  do
  ^^^^^^^^^^^^^^^^^^^^^^^^^ Non `public` test method detected. Make it `public` for it to run.
    assert_equal 42, do_something
  end
end
