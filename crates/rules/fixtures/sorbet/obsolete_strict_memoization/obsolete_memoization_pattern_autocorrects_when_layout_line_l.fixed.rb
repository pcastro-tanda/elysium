def foobar
  @foobar ||= T.let(Object.new, T.nilable(Object))
end
