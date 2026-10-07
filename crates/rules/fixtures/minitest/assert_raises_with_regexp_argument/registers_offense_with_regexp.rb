assert_raises(MyError, /some message/) do
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ Do not pass regular expression literals to `assert_raises`. Test the resulting exception.
  foo
end
