def a
  @b ||= :foo
  call_something_else
end
